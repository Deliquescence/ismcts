use std::rc::Rc;

pub trait Game: Clone {
    type Move: Clone;
    type Player: Clone;
    type MoveList: std::iter::IntoIterator<Item = Self::Move>;

    fn randomize_determination(&mut self, observer: &Self::Player);

    fn current_player(&self) -> &Self::Player;

    fn next_player(&self) -> &Self::Player;

    fn available_moves(&self) -> Self::MoveList;

    fn make_move(&mut self, mov: &Self::Move);

    fn result(&self, player: &Self::Player) -> Option<f64>;
}


struct Node<G: Game> {
    mov: G::Move,
    parent: Option<Rc<Node<G>>>,
    children: Vec<Node<G>>,
    player_just_moved: G::Player,
}

impl<G: Game> Node<G> {
    fn untried_moves(&self, legal_moves: G::MoveList) -> G::MoveList {
        unimplemented!();
    }

    fn select_child(&self, legal_moves: G::MoveList) -> Self {
        unimplemented!();
    }

    fn add_child(&mut self) {
        unimplemented!();
    }

    fn update(&mut self, result: f64) {
        unimplemented!();
    }
}


pub trait ISMCTS {

    fn select(&self);

}
