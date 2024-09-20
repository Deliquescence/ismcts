use crate::*;

#[derive(Clone, Debug, Default)]
struct TenMoveGame {
    moves: Vec<u8>,
}

const TOTAL_TURNS: usize = 2;
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

const ITERATIONS: usize = 1000;

#[test]
pub fn number_of_children_1_thread() {
    number_of_children(1);
}

#[test]
pub fn number_of_children_4_threads() {
    number_of_children(4);
}

fn number_of_children(n_threads: usize) {
    let game = TenMoveGame::default();
    let mut ismcts = IsmctsHandler::new(game);
    ismcts.run_iterations(n_threads, ITERATIONS);
    // ismcts.debug_children();

    let children = ismcts.root_node.children.read().unwrap();

    let total_iterations = ITERATIONS * n_threads;
    assert_eq!(10, children.len());
    assert_eq!(
        total_iterations,
        children
            .iter()
            .map(|c| c.statistics.read().unwrap().visit_count)
            .sum()
    );

    for child in children.iter() {
        // assert_eq!(
        //     total_iterations,
        //     child.statistics.read().unwrap().availability_count
        // );
        assert_eq!(10, child.children.read().unwrap().len());
    }
}
