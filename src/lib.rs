use rand::prelude::*;
use std::sync::{Arc, RwLock};

pub trait Game: Clone {
    type Move: Clone + PartialEq;
    type Player: Clone;
    type MoveList: Clone + std::iter::IntoIterator<Item = Self::Move>;

    fn randomize_determination(&mut self, observer: &Self::Player);

    fn current_player(&self) -> &Self::Player;

    fn next_player(&self) -> &Self::Player;

    fn environment_player(&self) -> &Self::Player;

    fn available_moves(&self) -> Self::MoveList;

    fn make_move(&mut self, mov: &Self::Move);

    fn result(&self, player: &Self::Player) -> Option<f64>;

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
    player_just_moved: Option<G::Player>,
    statistics: RwLock<NodeStatistics>,
}

#[derive(Debug, Default)]
struct NodeStatistics {
    visit_count: usize,
    availability_count: usize,
    reward: f64,
}

impl<G: Game> Node<G> {
    fn move_tried(&self, mov: &G::Move) -> bool {
        self.children
            .read()
            .unwrap()
            .iter()
            .any(|c| c.mov.as_ref().unwrap() == mov)
    }

    fn untried_moves(
        &self,
        legal_moves: &G::MoveList,
    ) -> impl std::iter::IntoIterator<Item = G::Move> + '_ {

        legal_moves
            .clone()
            .into_iter()
            .filter(|m| !self.move_tried(m))
            .collect::<Vec<_>>()
    }

    fn select_child(&self, legal_moves: &G::MoveList) -> Arc<Node<G>> {
        unimplemented!();
    }

    fn add_child(self: Arc<Self>, mov: G::Move, player: G::Player) -> Arc<Node<G>> {
        let p = Arc::clone(&self);
        let child = Arc::new(Node {
            mov: Some(mov),
            parent: Some(p),
            children: Default::default(),
            player_just_moved: Some(player),
            statistics: Default::default(),
        });
        self.children.write().unwrap().push(Arc::clone(&child));
        child
    }

    fn update(&self, terminal_state: &G) {
        let mut statistics = self.statistics.write().unwrap();

        statistics.visit_count += 1;
        if let Some(p) = &self.player_just_moved {
            statistics.reward += terminal_state.result(&p).unwrap_or_default();
        }
    }
}


pub trait ISMCTS<G: Game> {

    fn ismcts(&mut self, root_state: G, n_iterations: usize) {

        let root_node: Arc<Node<G>> = Arc::new(Node {
            mov: None,
            parent: None,
            children: Default::default(),
            player_just_moved: None,
            statistics: Default::default(),
        });

        for _i in 0..n_iterations {
            let mut rng = thread_rng();
            let mut state = root_state.clone();
            let mut node = Arc::clone(&root_node);

            // Determinize
            state.randomize_determination(root_state.current_player());

            // Select
            let mut available_moves = state.available_moves();
            while node
                .untried_moves(&available_moves)
                .into_iter()
                .next()
                .is_none()
            {
                node = node.select_child(&available_moves);
                state.make_move(&node.mov.clone().unwrap());
                available_moves = state.available_moves();
            }

            //Expand
            if let Some(m) = node
                .untried_moves(&available_moves)
                .into_iter()
                .choose(&mut rng)
            {
                let player = state.current_player().clone();
                state.make_move(&m);
                node = node.add_child(m, player);
            }

            //Backprop
            let mut backprop_node = Some(node);
            while let Some(n) = backprop_node {
                n.update(&state);
                backprop_node = n.parent.clone();
            }
        }

    }

}
