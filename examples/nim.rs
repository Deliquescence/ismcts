use ismcts::*;
use std::iter;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NimMode {
    Standard,
    Misere,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NimPlayer {
    First,
    Second,
}

#[derive(Clone, Debug)]
pub struct NimState {
    pub heaps: Vec<usize>,
    mode: NimMode,
    player_to_move: NimPlayer,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NimMove {
    pub heap: usize,
    pub amount: usize,
}

impl Game for NimState {
    type Move = NimMove;
    type Player = NimPlayer;
    type MoveList = Vec<NimMove>;

    fn randomize_determination(&mut self, _observer: &Self::Player) {
        //No-op, perfect information
    }

    fn current_player(&self) -> &Self::Player {
        &self.player_to_move
    }

    fn next_player(&self) -> &Self::Player {
        match self.player_to_move {
            NimPlayer::First => &NimPlayer::Second,
            NimPlayer::Second => &NimPlayer::First,
        }
    }

    fn available_moves(&self) -> Self::MoveList {
        self.heaps
            .iter()
            .enumerate()
            .filter(|(_i, amt)| **amt > 0)
            .flat_map(|(i, amt)| iter::repeat(i).zip((1..=*amt).into_iter()))
            .map(|(heap, amount)| NimMove { heap, amount })
            .collect()
    }

    fn make_move(&mut self, mov: &Self::Move) {
        if mov.heap >= self.heaps.len() {
            panic!("trying to move on out of bounds heap");
        }
        if mov.amount > self.heaps[mov.heap] {
            panic!("trying to take more than heap contains");
        }

        self.heaps[mov.heap] -= mov.amount;
        self.player_to_move = *self.next_player();
    }

    fn result(&self, player: &Self::Player) -> Option<f64> {
        if self.heaps.iter().any(|amt| *amt > 0) {
            None
        } else {
            let who_took_last = self.next_player(); //equivalent to previous player
            match self.mode {
                NimMode::Standard if player == who_took_last => Some(1.0),
                NimMode::Misere if player != who_took_last => Some(1.0),
                _ => Some(-1.0),
            }
        }
    }
}

pub struct NimIsmcts {}

impl ISMCTS<NimState> for NimIsmcts {}

pub fn main() {
    // Second player = human
    // Pretend the CPU moved first and got into a winning position
    // Test if ismcts can win
    let mut game = NimState {
        heaps: vec![3, 5, 6],
        mode: NimMode::Misere,
        player_to_move: NimPlayer::Second,
    };

    let read_num = || -> usize {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        input.trim().parse().unwrap_or_default()
    };

    while game.result(&NimPlayer::Second).is_none() {
        println!("{:?}", game);
        println!("Take from which heap (0-indexed):");
        let heap = read_num();
        println!("Take how many:");
        let amount = read_num();
        game.make_move(&NimMove { heap, amount });

		//CPU turn
		let mut ismcts = NimIsmcts {};
        let mov = ismcts.ismcts(game.clone(), 1000);
        println!("ISMCTS Move: {:?}", mov);
        game.make_move(&mov);
    }

    match game.result(&NimPlayer::Second) {
        Some(x) if x < 0.0 => println!("Human loses!"),
        Some(x) if x > 0.0 => println!("Human Wins!"),
        _ => unreachable!(),
    }
}
