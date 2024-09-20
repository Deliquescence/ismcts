use crate::*;

#[derive(Clone, Debug, Default)]
struct TenMoveGame {
    moves: Vec<u8>,
}

const TOTAL_TURNS: usize = 5;
impl Game for TenMoveGame {
    type Move = u8;

    type PlayerTag = usize;

    type MoveList = Vec<u8>;

    fn randomize_determination(&mut self, _observer: Self::PlayerTag) {}

    fn current_player(&self) -> Self::PlayerTag {
        self.moves.len() % 2
    }

    fn next_player(&self) -> Self::PlayerTag {
        (self.moves.len() + 1) % 2
    }

    fn available_moves(&self) -> Self::MoveList {
        if self.moves.len() > TOTAL_TURNS {
            Vec::new()
        } else {
            (0..10).collect()
        }
    }

    fn make_move(&mut self, mov: &Self::Move) {
        self.moves.push(*mov);
    }

    fn result(&self, player: Self::PlayerTag) -> Option<f64> {
        if self.moves.len() < TOTAL_TURNS {
            return None;
        }
        self.moves
            .last()
            .map(|m| if usize::from(*m) == player { 1.0 } else { 0.0 })
    }
}

#[test]
pub fn number_of_children_1_thread() {
    let game = TenMoveGame::default();
    let mut ismcts = IsmctsHandler::new(game);
    ismcts.run_iterations(1, 1000);

    let children = ismcts.root_node.children.read().unwrap();
    assert_eq!(10, children.len());
    for child in children.iter() {
        assert_eq!(10, child.children.read().unwrap().len());
    }
}

#[test]
pub fn number_of_children_4_threads() {
    let game = TenMoveGame::default();
    let mut ismcts = IsmctsHandler::new(game);
    ismcts.run_iterations(4, 1000);

    let children = ismcts.root_node.children.read().unwrap();
    assert_eq!(10, children.len());
    for child in children.iter() {
        assert_eq!(10, child.children.read().unwrap().len());
    }
}
