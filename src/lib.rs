use std::sync::{Arc, Mutex};

pub trait Game: Clone {
    type Move: Clone;
    type Player: Clone;
    type MoveList: std::iter::IntoIterator<Item = Self::Move>;

    fn randomize_determination(&mut self, observer: &Self::Player);

    fn current_player(&self) -> &Self::Player;

    fn next_player(&self) -> &Self::Player;

    fn environment_player(&self) -> &Self::Player;

    fn available_moves(&self) -> Self::MoveList;

    fn make_move(&mut self, mov: &Self::Move);

    fn result(&self, player: &Self::Player) -> Option<f64>;
}


struct Node<G: Game> {
    /// Move which entered this node
    mov: Option<G::Move>,
    parent: Option<Arc<Mutex<Node<G>>>>,
    children: Vec<Node<G>>,
    player_just_moved: G::Player,
}

impl<G: Game> Node<G> {
    fn untried_moves(&self, legal_moves: &G::MoveList) -> G::MoveList {
        unimplemented!();
    }

    fn select_child(&self, legal_moves: &G::MoveList) -> Self {
        unimplemented!();
    }

    fn add_child(mut parent: Arc<Mutex<Self>>, mov: G::Move, player: G::Player) {
        let p = Arc::clone(&parent);
        parent.lock().unwrap().children.push(Node {
            mov: Some(mov),
            parent: Some(p),
            children: Vec::new(),
            player_just_moved: player,
        });
    }

    fn update(&mut self, result: f64) {
        unimplemented!();
    }
}


pub trait ISMCTS<G: Game> {

    fn ismcts(&mut self, root_state: G, n_iterations: usize) {

        let root_node: Node<G> = Node {
            mov: None,
            parent: None,
            children: Vec::new(),
            player_just_moved: root_state.environment_player().clone(),
        };

        let mut node = root_node;
        for _i in 0..n_iterations {
            let mut state = root_state.clone();

            // Determinize
            state.randomize_determination(root_state.current_player());

            // Select
            let available_moves = state.available_moves();
            while let Some(_) = node
                .untried_moves(&available_moves)
                .into_iter()
                .next()
            {
                node = node.select_child(&available_moves);
                state.make_move(&node.mov.clone().unwrap());
            }

            //Expand

        }

    }

}
