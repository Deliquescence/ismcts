use crossbeam::thread;
use ordered_float::OrderedFloat;
use rand::prelude::*;
use std::marker::{Send, Sync};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

pub trait Game: Clone + Send + Sync {
    type Move: Clone + PartialEq + Send + Sync + std::fmt::Debug;
    type PlayerTag: Clone + Copy + Send + Sync + std::fmt::Debug;
    type MoveList: Clone + std::iter::IntoIterator<Item = Self::Move>;

    fn randomize_determination(&mut self, observer: Self::PlayerTag);

    fn current_player(&self) -> Self::PlayerTag;

    fn next_player(&self) -> Self::PlayerTag;

    fn available_moves(&self) -> Self::MoveList;

    fn make_move(&mut self, mov: &Self::Move);

    fn result(&self, player: Self::PlayerTag) -> Option<f64>;

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
    /// Move which entered this node
    mov: Option<G::Move>,
    parent: Option<Arc<Node<G>>>,
    children: RwLock<Vec<Arc<Node<G>>>>,
    player_just_moved: Option<G::PlayerTag>,
    statistics: RwLock<NodeStatistics>,
}

#[derive(Debug, Default)]
struct NodeStatistics {
    visit_count: usize,
    availability_count: usize,
    reward: f64,
}

impl NodeStatistics {
    pub fn ucb1(&self) -> f64 {
        (self.reward / self.visit_count as f64)
            + (2.0 * (self.availability_count as f64).ln() / self.visit_count as f64).sqrt()
    }
}

impl<G: Game> Node<G> {
    fn move_tried(&self, mov: &G::Move) -> bool {
        self.children
            .read()
            .unwrap()
            .iter()
            .any(|c| c.mov.as_ref().unwrap() == mov)
    }

    fn untried_moves<M>(&self, legal_moves: &M) -> impl std::iter::IntoIterator<Item = G::Move> + '_
    where
        M: Clone + std::iter::IntoIterator<Item = G::Move>,
    {
        legal_moves
            .clone()
            .into_iter()
            .filter(|m| !self.move_tried(m))
            .collect::<Vec<_>>()
    }

    fn select_child<M>(&self, legal_moves: &M) -> Option<Arc<Node<G>>>
    where
        M: Clone + std::iter::IntoIterator<Item = G::Move>,
    {
        let legal_moves: Vec<_> = legal_moves.clone().into_iter().collect();
        let children = self.children.read().unwrap();
        let legal_children = children
            .iter()
            .filter(|c| legal_moves.iter().any(|m| c.mov.as_ref().unwrap() == m));

        let choice = legal_children
            .clone()
            .max_by_key(|c| OrderedFloat::from(c.statistics.read().unwrap().ucb1()))
            .cloned();
        //Update availibility count now
        legal_children.for_each(|c| c.statistics.write().unwrap().availability_count += 1);
        choice
    }

    fn add_child(self: Arc<Self>, mov: G::Move, player_tag: G::PlayerTag) -> Arc<Node<G>> {
        let p = Arc::clone(&self);
        let child = Arc::new(Node {
            mov: Some(mov),
            parent: Some(p),
            children: Default::default(),
            player_just_moved: Some(player_tag),
            statistics: Default::default(),
        });
        self.children.write().unwrap().push(Arc::clone(&child));
        child
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
}

impl<G: Game> IsmctsHandler<G> {
    pub fn new(root_state: G) -> Self {
        let root_node = Arc::new(Node {
            mov: None,
            parent: None,
            children: Default::default(),
            player_just_moved: None,
            statistics: Default::default(),
        });
        IsmctsHandler {
            root_state, //todo implement a way to make a move and advance down the tree
            root_node,
        }
    }

    pub fn run_iterations(&mut self, n_threads: usize, n_iterations_per_thread: usize) {
        spawn_n_threads(n_threads, |_| {
            ismcts_work_thread_iterations(
                self.root_state.clone(),
                Arc::clone(&self.root_node),
                n_iterations_per_thread,
            )
        });
    }

    pub fn run_timed(&mut self, n_threads: usize, time: Duration) {
        spawn_n_threads(n_threads, |_| {
            ismcts_work_thread_timed(self.root_state.clone(), Arc::clone(&self.root_node), time)
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

            node = node.select_child(&available_moves).unwrap();
            state.make_move(&node.mov.clone().unwrap());
            available_moves = state.available_moves().into_iter().collect();
            depth += 1;
        }
    }

    pub fn debug_children(&self) {
        for c in self.root_node.children.read().unwrap().iter() {
            dbg!(&c.mov);
            dbg!(&*c.statistics.read().unwrap());
        }
        let max_visit_count: usize = self
            .root_node
            .children
            .read()
            .unwrap()
            .iter()
            .map(|c| c.statistics.read().unwrap().visit_count)
            .max()
            .unwrap();
        println!("Max visit count: {}", max_visit_count)
    }
}

fn ismcts_one_iteration<G: Game>(mut state: G, mut node: Arc<Node<G>>) {
    let mut rng = thread_rng();

    // Determinize
    state.randomize_determination(state.current_player());

    // Select
    let mut available_moves: Vec<_> = state.available_moves().into_iter().collect();
    while !available_moves.is_empty()
        && node
            .untried_moves(&available_moves)
            .into_iter()
            .next()
            .is_none()
    {
        node = node.select_child(&available_moves).unwrap();
        state.make_move(&node.mov.clone().unwrap());
        available_moves = state.available_moves().into_iter().collect();
    }

    //Expand
    if let Some(m) = node
        .untried_moves(&available_moves)
        .into_iter()
        .choose(&mut rng)
    {
        let player_tag = state.current_player();
        state.make_move(&m);
        node = node.add_child(m, player_tag);
    }

    //Simulate
    state.random_rollout();

    //Backprop
    let mut backprop_node = Some(node);
    while let Some(n) = backprop_node {
        n.update(&state);
        backprop_node = n.parent.clone();
    }
}

fn ismcts_work_thread_iterations<G: Game>(
    root_state: G,
    root_node: Arc<Node<G>>,
    n_iterations: usize,
) {
    for _i in 0..n_iterations {
        let state = root_state.clone();
        let node = Arc::clone(&root_node);

        ismcts_one_iteration(state, node);
    }
}

fn ismcts_work_thread_timed<G: Game>(root_state: G, root_node: Arc<Node<G>>, time: Duration) {
    let start = Instant::now();
    loop {
        let duration = start.elapsed();
        if duration > time {
            break;
        }
        let state = root_state.clone();
        let node = Arc::clone(&root_node);

        ismcts_one_iteration(state, node);
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
