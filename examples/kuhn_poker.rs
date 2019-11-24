use ismcts::*;
use rand::prelude::*;
use std::cmp::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KPCard {
    King,
    Queen,
    Jack,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KPPlayer {
    First,
    Second,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum KPMove {
    Check,
    Bet,
    Fold,
    Call,
}

#[derive(Clone, Debug)]
pub struct KPState {
    first_player_card: KPCard,
    second_player_card: KPCard,
    move_history: Vec<KPMove>,
    first_player_bank: isize,
    second_player_bank: isize,
    pot: isize,
    game_over: bool,
}

impl KPState {
    pub fn new() -> Self {
        let first_player_card = KPCard::random_sample();
        let second_player_card = KPCard::random_sample_neq_other(first_player_card);

        KPState {
            first_player_card,
            second_player_card,
            move_history: Vec::new(),
            // Each player antes 1
            first_player_bank: -1,
            second_player_bank: -1,
            pot: 2,
            game_over: false,
        }
    }

    pub fn victor(&self) -> KPPlayer {
        match self.first_player_card.cmp(&self.second_player_card) {
            Ordering::Less => KPPlayer::Second,
            Ordering::Greater => KPPlayer::First,
            Ordering::Equal => panic!("Deck has 1 of each card"),
        }
    }

    fn award_pot(&mut self, player: KPPlayer) {
        match player {
            KPPlayer::First => self.first_player_bank += self.pot,
            KPPlayer::Second => self.second_player_bank += self.pot,
        }
        self.pot = 0;
        self.game_over = true;
    }

    fn player_bet(&mut self, player: KPPlayer) {
        match player {
            KPPlayer::First => self.first_player_bank -= 1,
            KPPlayer::Second => self.second_player_bank -= 1,
        }
        self.pot += 1;
    }
}

impl Game for KPState {
    type Move = KPMove;
    type PlayerTag = KPPlayer;
    type MoveList = Vec<KPMove>;

    fn randomize_determination(&mut self, observer: Self::PlayerTag) {
        match observer {
            KPPlayer::First => {
                self.second_player_card = KPCard::random_sample_neq_other(self.first_player_card)
            }
            KPPlayer::Second => {
                self.first_player_card = KPCard::random_sample_neq_other(self.second_player_card)
            }
        }
    }

    fn current_player(&self) -> Self::PlayerTag {
        match self.move_history.len() % 2 {
            0 => KPPlayer::First,
            1 => KPPlayer::Second,
            _ => unreachable!(),
        }
    }

    fn next_player(&self) -> Self::PlayerTag {
        match self.current_player() {
            KPPlayer::First => KPPlayer::Second,
            KPPlayer::Second => KPPlayer::First,
        }
    }

    fn available_moves(&self) -> Self::MoveList {
        match self.move_history.len() {
            0 => vec![KPMove::Check, KPMove::Bet],
            1 => match self.move_history[0] {
                KPMove::Check => vec![KPMove::Check, KPMove::Bet],
                KPMove::Bet => vec![KPMove::Fold, KPMove::Call],
                _ => panic!("illegal first move was made"),
            },
            2 => match self.move_history[1] {
                KPMove::Fold | KPMove::Call => Vec::new(),
                KPMove::Check => Vec::new(),
                KPMove::Bet => vec![KPMove::Fold, KPMove::Call],
            },
            _ => Vec::new(),
        }
    }

    fn make_move(&mut self, mov: &Self::Move) {
        assert!(
            self.move_history.len() <= 2,
            "making move when game is over"
        );
        match mov {
            KPMove::Check if self.move_history.get(0) == Some(&KPMove::Check) => {
                self.award_pot(self.victor())
            }
            KPMove::Check => (),
            KPMove::Fold => self.award_pot(self.next_player()),
            KPMove::Bet => self.player_bet(self.current_player()),
            KPMove::Call => {
                self.player_bet(self.current_player());
                self.award_pot(self.victor());
            }
        }
        self.move_history.push(*mov);
    }

    fn result(&self, player: Self::PlayerTag) -> Option<f64> {
        if !self.game_over {
            None
        } else if player == KPPlayer::First {
            Some(self.first_player_bank as f64)
        } else {
            Some(self.second_player_bank as f64)
        }
    }
}

pub fn main() {
    // playout_once(true);

    let n = 10000;
    let mut results = 0.0;
    for _ in 0..n {
        let result = playout_once(false);
        results += result;
    }

    println!("ISMCTS average result: {:?}", results / n as f64);
}

pub fn playout_once(verbose: bool) -> f64 {
    let mut state = KPState::new();
    if verbose {
        dbg!(&state);
    }

    let mov = ismcts_policy(&mut state.clone()).unwrap();
    if verbose {
        println!("ISMCTS move: {:?}", mov);
    }
    state.make_move(&mov);

    let mov = second_player_equilibruim_policy(&state).unwrap();
    if verbose {
        println!("Second player move: {:?}", mov);
    }
    state.make_move(&mov);

    if let Some(mov) = ismcts_policy(&mut state.clone()) {
        if verbose {
            println!("ISMCTS move: {:?}", mov);
        }
        state.make_move(&mov);
    }

    match state.result(KPPlayer::First) {
        Some(x) if x < 0.0 => {
            if verbose {
                println!("ISMCTS Loses {:?}!", x);
            }
            x
        }
        Some(x) if x > 0.0 => {
            if verbose {
                println!("ISMCTS Wins {:?}!", x);
            }
            x
        }
        _ => unreachable!(),
    }
}

pub fn ismcts_policy(state: &mut KPState) -> Option<KPMove> {
    let mut ismcts = IsmctsHandler::new(state.clone());
    ismcts.run_iterations(4, 10000 / 4);
    ismcts.best_move()
}

pub fn second_player_equilibruim_policy(state: &KPState) -> Option<KPMove> {
    if state.move_history.len() != 1 {
        None
    } else {
        //Second players options are always either check/bet or call/fold
        let check_bet = match state.move_history[0] {
            KPMove::Check => true,
            KPMove::Bet => false,
            _ => panic!("illegal first move was made"),
        };
        let mut rng = thread_rng();
        let one_third = rng.gen_bool(1.0 / 3.0); //error[E0301]: cannot mutably borrow in a pattern guard
        match state.second_player_card {
            KPCard::King if check_bet => Some(KPMove::Bet),
            KPCard::King => Some(KPMove::Call),
            KPCard::Queen if check_bet => Some(KPMove::Check),
            KPCard::Queen if one_third => Some(KPMove::Call),
            KPCard::Queen => Some(KPMove::Fold),
            KPCard::Jack if check_bet && one_third => Some(KPMove::Bet),
            KPCard::Jack if check_bet => Some(KPMove::Check),
            KPCard::Jack => Some(KPMove::Fold),
        }
    }
}

impl PartialOrd for KPCard {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for KPCard {
    fn cmp(&self, other: &Self) -> Ordering {
        match self {
            KPCard::King if *other == KPCard::King => Ordering::Equal,
            KPCard::King => Ordering::Greater,
            KPCard::Jack if *other == KPCard::Jack => Ordering::Equal,
            KPCard::Jack => Ordering::Less,
            KPCard::Queen => match other {
                KPCard::King => Ordering::Less,
                KPCard::Jack => Ordering::Greater,
                KPCard::Queen => Ordering::Equal,
            },
        }
    }
}

impl Distribution<KPCard> for rand::distributions::Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> KPCard {
        match rng.gen_range(0, 3) {
            0 => KPCard::King,
            1 => KPCard::Queen,
            _ => KPCard::Jack,
        }
    }
}

impl KPCard {
    pub fn random_sample() -> Self {
        rand::random()
    }

    pub fn random_sample_neq_other(other: Self) -> Self {
        loop {
            let c: KPCard = rand::random();
            if c != other {
                return c;
            }
        }
    }
}
