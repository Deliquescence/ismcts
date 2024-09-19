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

impl NimState {
    pub fn get_largest_heap(&self) -> Option<(usize, usize)> {
        self.heaps
            .iter()
            .enumerate()
            .max_by_key(|(_i, &c)| c)
            .map(|(i, &c)| (i, c))
    }
}

impl Game for NimState {
    type Move = NimMove;
    type PlayerTag = NimPlayer;
    type MoveList = Vec<NimMove>;

    fn randomize_determination(&mut self, _observer: Self::PlayerTag) {
        //No-op
    }

    fn current_player(&self) -> Self::PlayerTag {
        self.player_to_move
    }

    fn next_player(&self) -> Self::PlayerTag {
        match self.player_to_move {
            NimPlayer::First => NimPlayer::Second,
            NimPlayer::Second => NimPlayer::First,
        }
    }

    fn available_moves(&self) -> Self::MoveList {
        self.heaps
            .iter()
            .enumerate()
            .filter(|(_i, amt)| **amt > 0)
            .flat_map(|(i, amt)| iter::repeat(i).zip(1..=*amt))
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
        self.player_to_move = self.next_player();
    }

    fn result(&self, player: Self::PlayerTag) -> Option<f64> {
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

pub fn human_move(game: &mut NimState) -> NimMove {
    let read_num = || -> usize {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        input.trim().parse().unwrap_or_default()
    };

    println!("{:?}", &game);
    println!("Take from which heap (0-indexed):");
    let heap = read_num();
    println!("Take how many:");
    let amount = read_num();
    NimMove { heap, amount }
}

pub fn perfect_move(game: &NimState) -> NimMove {
    // https://en.wikipedia.org/wiki/Nim#Example_implementation

    let is_endgame = game.heaps.iter().filter(|c| **c > 1).count() <= 1;
    if is_endgame && game.mode == NimMode::Misere {
        let n_remaining_heaps = game.heaps.iter().filter(|c| **c > 0).count();
        let (i_max, c_max) = game.get_largest_heap().unwrap();
        if c_max == 1 && n_remaining_heaps % 2 == 1 {
            //Losing position
            println!("losing position");
            NimMove {
                heap: i_max,
                amount: 1,
            }
        } else {
            NimMove {
                heap: i_max,
                amount: c_max - (n_remaining_heaps % 2),
            }
        }
    } else {
        let nim_sum = game.heaps.iter().fold(0, |acc, amt| acc ^ amt);
        if nim_sum == 0 {
            //Losing position
            println!("losing position");
            let (i_max, _c_max) = game.get_largest_heap().unwrap();
            NimMove {
                heap: i_max,
                amount: 1,
            }
        } else {
            game.heaps
                .iter()
                .enumerate()
                .find_map(|(i, amount)| {
                    let target_size = amount ^ nim_sum;
                    if target_size < *amount {
                        Some(NimMove {
                            heap: i,
                            amount: amount - target_size,
                        })
                    } else {
                        None
                    }
                })
                .unwrap()
        }
    }
}

pub fn main() {
    maintain_win();
    // let game = NimState {
    //     heaps: vec![3, 5, 6, 100],
    //     mode: NimMode::Misere,
    //     player_to_move: NimPlayer::Second,
    // };

    // let mut ismcts = IsmctsHandler::new(game.clone());
    // ismcts.run_iterations(1, 100_000);
    // ismcts.debug_max_visits();
}

const N_THREADS: usize = 8;

#[allow(dead_code)]
fn maintain_win() {
    let heaps = vec![3, 5, 6];
    let nim_sum = heaps.iter().fold(0, |acc, amt| acc ^ amt);

    let game = NimState {
        heaps,
        mode: NimMode::Misere,
        // Winning strategy is to end turn with nim sum 0
        player_to_move: if nim_sum == 0 {
            // Perfect algorithm plays the losing side
            NimPlayer::Second
        } else {
            // Allow ISMCTS to play from the winning position
            NimPlayer::First
        },
    };

    let mut ismcts = IsmctsHandler::new(game);
    while ismcts.state().result(NimPlayer::First).is_none() {
        ismcts.run_iterations(N_THREADS, ismcts.state().available_moves().len());

        println!("{:?}", &ismcts.state());
        let perfect_mov = perfect_move(ismcts.state());
        println!("Perfect move: {:?}", perfect_mov);

        if ismcts.state().current_player() == NimPlayer::First {
            ismcts.run_iterations(N_THREADS, 100000);
            let mov = ismcts.best_move().unwrap();
            ismcts.debug_children();

            println!("ISMCTS move: {:?}", mov);
            ismcts.make_move(&mov);
        } else {
            ismcts.make_move(&perfect_mov);
        }
        println!();
    }

    match ismcts.state().result(NimPlayer::First) {
        Some(x) if x < 0.0 => println!("ISMCTS Loses!"),
        Some(x) if x > 0.0 => println!("ISMCTS Wins!"),
        _ => unreachable!(),
    }
}
