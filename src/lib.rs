use crossbeam::thread;
use ordered_float::OrderedFloat;
use palette::{LinSrgb, Mix};
use rand::prelude::*;
use std::marker::{Send, Sync};
use std::sync::{Arc, RwLock, Weak};
use std::time::{Duration, Instant};
use uuid::Uuid;

#[cfg(test)]
mod tests;

pub trait Game: Clone + Send + Sync {
    type Move: Clone + PartialEq + Send + Sync + std::fmt::Debug;
    type PlayerTag: Clone + Copy + PartialEq + Send + Sync + std::fmt::Debug;
    type MoveList: Clone + std::iter::IntoIterator<Item = Self::Move>;

    fn randomize_determination(&mut self, observer: Self::PlayerTag);

    fn current_player(&self) -> Self::PlayerTag;

    fn next_player(&self) -> Self::PlayerTag;

    fn available_moves(&self) -> Self::MoveList;

    fn make_move(&mut self, mov: &Self::Move);

    fn result(&self, player: Self::PlayerTag) -> Option<f64>;

    /// Optional: Get prior probabilities for moves from a policy network
    /// Returns a vector of (move, probability) pairs
    /// If not implemented, defaults to uniform priors
    fn move_probabilities(&self) -> Option<Vec<(Self::Move, f64)>> {
        None
    }

    /// Optional: Get value estimate from a value network for current position
    ///
    /// Returns expected value from current player's perspective in range [-1, 1]
    /// If not implemented, falls back to random rollout
    ///
    /// # Two-Player Zero-Sum Assumption
    ///
    /// When using value networks, the implementation assumes two-player zero-sum games.
    /// The value is automatically negated when backpropagating to opponent nodes.
    /// For non-zero-sum games, use rollouts instead (don't implement this method).
    ///
    /// # Example
    ///
    /// If current player is Player A and the value estimate is +0.8:
    /// - Nodes where Player A moved will receive +0.8
    /// - Nodes where Player B moved will receive -0.8
    fn value_estimate(&self) -> Option<f64> {
        None
    }

    fn random_rollout(&mut self) {
        let mut rng = thread_rng();
        while self.result(self.current_player()).is_none() {
            let mov = self.available_moves().into_iter().choose(&mut rng);
            if let Some(m) = mov {
                self.make_move(&m);
            } else {
                break;
            }
        }
    }
}

struct Node<G: Game> {
    /// Move which entered this node (None if not generating IDs)
    id: Option<String>,
    mov: Option<G::Move>,
    parent: Option<Weak<Node<G>>>,
    children: RwLock<Vec<Arc<Node<G>>>>,
    player_just_moved: Option<G::PlayerTag>,
    statistics: RwLock<NodeStatistics>,
    generate_node_ids: bool,
}

#[derive(Debug)]
struct NodeStatistics {
    visit_count: usize,
    availability_count: usize,
    reward: f64,
    prior_probability: f64,
}

impl Default for NodeStatistics {
    fn default() -> Self {
        NodeStatistics {
            visit_count: 0,
            availability_count: 0,
            reward: 0.0,
            prior_probability: 1.0, // Default uniform prior
        }
    }
}

impl NodeStatistics {
    /// PUCT formula (used in AlphaGo/AlphaZero)
    /// Q + c_puct * P * sqrt(N_parent) / (1 + N)
    /// where Q is average reward, P is prior probability, N is visit count
    pub fn puct(&self, parent_visits: usize, c_puct: f64) -> f64 {
        let q_value = if self.visit_count > 0 {
            self.reward / self.visit_count as f64
        } else {
            0.0
        };
        let exploration = c_puct * self.prior_probability * (parent_visits as f64).sqrt()
            / (1.0 + self.visit_count as f64);
        q_value + exploration
    }

    /// Fallback to UCB1 if priors are not available
    pub fn ucb1(&self) -> f64 {
        (self.reward / self.visit_count as f64)
            + (2.0 * (self.availability_count as f64).ln() / self.visit_count as f64).sqrt()
    }
}

impl<G: Game> Node<G> {
    fn untried_moves(&self, legal_moves: &[G::Move]) -> Vec<G::Move> {
        let children = self.children.read().unwrap();
        legal_moves
            .iter()
            .filter(|mov: &&G::Move| !children.iter().any(|c| c.mov.as_ref().unwrap() == *mov))
            .cloned()
            .collect::<Vec<_>>()
    }

    fn select_child(&self, legal_moves: &[G::Move], c_puct: f64) -> Option<Arc<Node<G>>> {
        let children = self.children.read().unwrap();
        let legal_children: Vec<_> = children
            .iter()
            .filter(|c| legal_moves.iter().any(|m| c.mov.as_ref().unwrap() == m))
            .collect(); // Need to enumerate twice

        let parent_visits = self.statistics.read().unwrap().visit_count;

        let choice = legal_children
            .iter()
            .max_by_key(|c| {
                let stats = c.statistics.read().unwrap();
                // Use PUCT if we have policy priors, otherwise fall back to UCB1
                if stats.prior_probability < 1.0 {
                    OrderedFloat::from(stats.puct(parent_visits, c_puct))
                } else {
                    OrderedFloat::from(stats.ucb1())
                }
            })
            .cloned();
        // To avoid backprop needing to recalculate/store which nodes were available, update availablity count now
        legal_children
            .iter()
            .for_each(|c| c.statistics.write().unwrap().availability_count += 1);
        choice.cloned()
    }

    fn add_child(
        self: Arc<Self>,
        mov: G::Move,
        player_tag: G::PlayerTag,
        prior: f64,
    ) -> Arc<Node<G>> {
        // Obtain a write lock on children to ensure that no other thread can add a child at the same time
        let mut children = self.children.write().unwrap();

        // Check if the child with the same move already exists (race condition prevention)
        if let Some(existing_child) = children.iter().find(|c| c.mov.as_ref() == Some(&mov)) {
            return Arc::clone(existing_child);
        }

        let p = Arc::downgrade(&self);
        let generate_node_ids = self.generate_node_ids;
        let child = Arc::new(Node {
            id: if generate_node_ids {
                Some(Uuid::new_v4().to_string())
            } else {
                None
            },
            mov: Some(mov),
            parent: Some(p),
            children: Default::default(),
            player_just_moved: Some(player_tag),
            statistics: RwLock::new(NodeStatistics {
                // We update the availabilty count during selection instead of backprop,
                // but the visit count _is_ updated during backprop, so the availability
                // of the new node needs a +1 because expansion happens after selection.
                availability_count: 1,
                prior_probability: prior,
                ..Default::default()
            }),
            generate_node_ids,
        });

        children.push(Arc::clone(&child));
        child
    }

    fn update_with_value(&self, value: f64) {
        let mut statistics = self.statistics.write().unwrap();
        statistics.visit_count += 1;
        statistics.reward += value;
    }

    fn update(&self, terminal_state: &G) {
        let mut statistics = self.statistics.write().unwrap();
        statistics.visit_count += 1;
        if let Some(p) = &self.player_just_moved {
            statistics.reward += terminal_state.result(*p).unwrap_or_default();
        }
    }
}

pub struct IsmctsHandler<G: Game> {
    root_state: G,
    root_node: Arc<Node<G>>,
    c_puct: f64, // Exploration constant for PUCT formula
}

impl<G: Game> IsmctsHandler<G> {
    pub fn new(root_state: G) -> Self {
        Self::new_with_config(root_state, 1.0, false)
    }

    pub fn new_with_puct(root_state: G, c_puct: f64) -> Self {
        Self::new_with_config(root_state, c_puct, false)
    }

    pub fn new_with_graph_support(root_state: G) -> Self {
        Self::new_with_config(root_state, 1.0, true)
    }

    pub fn new_with_config(root_state: G, c_puct: f64, generate_node_ids: bool) -> Self {
        let root_node = Arc::new(Node {
            id: if generate_node_ids {
                Some(Uuid::new_v4().to_string())
            } else {
                None
            },
            mov: None,
            parent: None,
            children: Default::default(),
            player_just_moved: None,
            statistics: Default::default(),
            generate_node_ids,
        });
        IsmctsHandler {
            root_state,
            root_node,
            c_puct,
        }
    }

    pub fn make_move(&mut self, mov: &G::Move) {
        assert!(
            self.root_state
                .available_moves()
                .into_iter()
                .any(|m| m == *mov),
            "Move must be legal"
        );
        let node = {
            let children = self.root_node.children.read().unwrap();
            let child_node = children.iter().find(|c| c.mov.as_ref() == Some(mov));
            assert!(child_node.is_some(), "Move must be explored");
            Arc::clone(child_node.unwrap())
        };

        self.root_state.make_move(mov);
        self.root_node = node;
    }

    pub fn run_iterations(&mut self, n_threads: usize, n_iterations_per_thread: usize) {
        let c_puct = self.c_puct;
        spawn_n_threads(n_threads, |_| {
            ismcts_work_thread_iterations(
                self.root_state.clone(),
                Arc::clone(&self.root_node),
                n_iterations_per_thread,
                c_puct,
            )
        });
    }

    pub fn run_timed(&mut self, n_threads: usize, time: Duration) {
        let c_puct = self.c_puct;
        spawn_n_threads(n_threads, |_| {
            ismcts_work_thread_timed(
                self.root_state.clone(),
                Arc::clone(&self.root_node),
                time,
                c_puct,
            )
        });
    }

    pub fn best_move(&self) -> Option<G::Move> {
        let children = self.root_node.children.read().unwrap();
        children
            .iter()
            .max_by_key(|c| c.statistics.read().unwrap().visit_count)
            .map(|c| c.mov.clone().unwrap())
    }

    pub fn debug_select(&self) {
        let mut node = Arc::clone(&self.root_node);
        let mut state = self.root_state.clone();
        let mut available_moves: Vec<_> = state.available_moves().into_iter().collect();
        let mut depth = 0;
        while !available_moves.is_empty()
            && node
                .untried_moves(&available_moves)
                .into_iter()
                .next()
                .is_none()
        {
            println!("DEPTH {}", depth);
            dbg!(&node.mov);
            dbg!(&node.statistics.read().unwrap());

            node = node.select_child(&available_moves, self.c_puct).unwrap();
            state.make_move(&node.mov.clone().unwrap());
            available_moves = state.available_moves().into_iter().collect();
            depth += 1;
        }
    }

    pub fn debug_children(&self) {
        let mut children: Vec<_> = self
            .root_node
            .children
            .read()
            .unwrap()
            .iter()
            .cloned()
            .collect();
        children.sort_by_key(|c| c.statistics.read().unwrap().visit_count);
        for c in children {
            let statistics = c.statistics.read().unwrap();
            dbg!(&c.mov);
            dbg!(&*statistics);
            dbg!(statistics.ucb1());
            println!();
        }
    }

    pub fn max_visits(&self) -> usize {
        self.root_node
            .children
            .read()
            .unwrap()
            .iter()
            .map(|c| c.statistics.read().unwrap().visit_count)
            .max()
            .unwrap_or_default()
    }

    pub fn total_visits(&self) -> usize {
        self.root_node
            .children
            .read()
            .unwrap()
            .iter()
            .map(|c| c.statistics.read().unwrap().visit_count)
            .sum::<usize>()
    }

    pub fn debug_max_visits(&self) {
        println!("Max visit count: {}", self.max_visits());
    }

    pub fn state(&self) -> &G {
        &self.root_state
    }

    pub fn dotty_graph(&self) {
        println!("{}", self.dotty_graph_string());
    }

    pub fn dotty_graph_string(&self) -> String {
        let mut output = String::new();
        output.push_str(
            r#"digraph G {
            rankdir="LR";
            node[label=""];
            node[shape=circle,size=100,style=filled];
            "#,
        );
        self.dotty_all_children_string(&self.root_node, &mut output);
        output.push_str("}\n");
        output
    }

    fn get_max_visits(&self, nodes: Vec<Arc<Node<G>>>) -> usize {
        let mut max_visits = 0;

        for node in nodes {
            max_visits = std::cmp::max(node.statistics.read().unwrap().visit_count, max_visits);
        }
        max_visits
    }

    fn dotty_all_children_string(&self, root_node: &Arc<Node<G>>, output: &mut String) {
        let mut nodes: Vec<Arc<Node<G>>> = vec![root_node.clone()];
        while !nodes.is_empty() {
            let mut next_nodes = vec![];
            let max_visits = self.get_max_visits(nodes.clone());
            for node in nodes {
                let visit_count = node.statistics.read().unwrap().visit_count;
                output.push_str(&format!(
                    r#""{}" [fillcolor="{}",label="{}"];
"#,
                    node.id.as_ref().unwrap(),
                    self.dotty_node_color(&node, max_visits),
                    visit_count,
                ));
                for child in node.children.read().unwrap().clone() {
                    if child.statistics.read().unwrap().visit_count == 1 {
                        continue;
                    }
                    next_nodes.push(child.clone());
                    output.push_str(&format!(
                        r#""{}" -> "{}";
"#,
                        node.id.as_ref().unwrap(),
                        child.id.as_ref().unwrap()
                    ));
                }
            }
            nodes = next_nodes;
        }
    }

    fn dotty_node_color(&self, node: &Arc<Node<G>>, max_visits: usize) -> String {
        let visit_count = node.statistics.read().unwrap().visit_count;
        if visit_count == 1 {
            "#ffffff".to_string()
        } else {
            color_gradient(visit_count as f32 / max_visits as f32)
        }
    }
}

fn color_gradient(t: f32) -> String {
    // Define the start and end colors
    let red = LinSrgb::new(1.0, 0.0, 0.0);
    let green = LinSrgb::new(0.0, 1.0, 0.0);

    // Interpolate between the colors
    let mixed_color = red.mix(&green, t);

    // Convert the color to hexadecimal format
    let (r, g, b) = mixed_color.into_components();
    format!(
        "#{:02X}{:02X}{:02X}",
        (r * 255.0) as u8,
        (g * 255.0) as u8,
        (b * 255.0) as u8
    )
}

fn ismcts_one_iteration<G: Game>(mut state: G, mut node: Arc<Node<G>>, c_puct: f64) {
    let mut rng = thread_rng();

    // Determinize
    state.randomize_determination(state.current_player());

    // Select
    let mut available_moves: Vec<_>;
    let mut untried_moves;
    loop {
        available_moves = state.available_moves().into_iter().collect();
        untried_moves = node.untried_moves(&available_moves);
        if available_moves.is_empty() || !untried_moves.is_empty() {
            break;
        }
        node = node.select_child(&available_moves, c_puct).unwrap();
        state.make_move(&node.mov.clone().unwrap());
    }

    //Expand
    if let Some(m) = untried_moves.into_iter().choose(&mut rng) {
        let player_tag = state.current_player();

        // Get policy priors from the game state if available
        let prior = if let Some(move_probs) = state.move_probabilities() {
            // Find the prior for this move
            move_probs
                .iter()
                .find(|(mov, _)| *mov == m)
                .map(|(_, p)| *p)
                .unwrap_or(1.0 / available_moves.len() as f64)
        } else {
            // No policy network - use default prior (1.0) to maintain UCB1 behavior
            1.0
        };

        state.make_move(&m);
        node = node.add_child(m, player_tag, prior);
    }

    //Simulate - use value network if available, otherwise rollout
    if let Some(v) = state.value_estimate() {
        // Use value network estimate
        // Value is from current player's perspective (who is about to move)
        let value_player = state.current_player();

        // Backprop with perspective conversion for two-player zero-sum games
        let mut backprop_node = node;
        loop {
            // For each node, convert value to that node's player's perspective
            let node_value = match &backprop_node.player_just_moved {
                Some(p) if *p == value_player => v, // Same player: use value as-is
                Some(_) => -v, // Different player: negate (assumes two-player zero-sum)
                None => v,     // Root node: use value as-is
            };

            backprop_node.update_with_value(node_value);

            let parent = backprop_node.parent.as_ref().and_then(Weak::upgrade);
            if let Some(n) = parent {
                backprop_node = n;
            } else {
                break;
            }
        }
    } else {
        // Fall back to random rollout
        // For rollouts, we query the terminal state for each player individually
        state.random_rollout();

        // Backprop using terminal state (each node gets its player's result)
        let mut backprop_node = node;
        loop {
            backprop_node.update(&state);

            let parent = backprop_node.parent.as_ref().and_then(Weak::upgrade);
            if let Some(n) = parent {
                backprop_node = n;
            } else {
                break;
            }
        }
    }
}

fn ismcts_work_thread_iterations<G: Game>(
    root_state: G,
    root_node: Arc<Node<G>>,
    n_iterations: usize,
    c_puct: f64,
) {
    for _i in 0..n_iterations {
        let state = root_state.clone();
        let node = Arc::clone(&root_node);

        ismcts_one_iteration(state, node, c_puct);
    }
}

fn ismcts_work_thread_timed<G: Game>(
    root_state: G,
    root_node: Arc<Node<G>>,
    time: Duration,
    c_puct: f64,
) {
    let start = Instant::now();
    loop {
        let duration = start.elapsed();
        if duration > time {
            break;
        }
        let state = root_state.clone();
        let node = Arc::clone(&root_node);

        ismcts_one_iteration(state, node, c_puct);
    }
}

fn spawn_n_threads<'env, F, T>(n_threads: usize, f: F)
where
    F: Copy + FnOnce(&crossbeam::thread::Scope<'env>) -> T + Send + 'env,
    T: Send + 'env,
{
    thread::scope(|s| {
        for _ in 0..n_threads {
            s.spawn(f);
        }
    })
    .unwrap();
}
