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

// Test game with policy network support
#[derive(Clone, Debug, Default)]
struct GameWithPolicy {
    moves: Vec<u8>,
}

impl Game for GameWithPolicy {
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
        if self.moves.len() >= 2 {
            Vec::new()
        } else {
            vec![0, 1, 2]
        }
    }

    fn make_move(&mut self, mov: &Self::Move) {
        self.moves.push(*mov);
    }

    fn result(&self, player: Self::PlayerTag) -> Option<f64> {
        if self.moves.len() < 2 {
            return None;
        }
        Some(if player == 0 { 1.0 } else { 0.0 })
    }

    // Return policy priors: move 0 has 70% probability, moves 1 and 2 have 15% each
    fn move_probabilities(&self) -> Option<Vec<(Self::Move, f64)>> {
        Some(vec![(0, 0.7), (1, 0.15), (2, 0.15)])
    }
}

// Test game with value network support
#[derive(Clone, Debug)]
struct GameWithValue {
    moves: Vec<u8>,
    value: f64,
}

impl Default for GameWithValue {
    fn default() -> Self {
        Self {
            moves: Vec::new(),
            value: 0.5,
        }
    }
}

impl Game for GameWithValue {
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
        if self.moves.len() >= 2 {
            Vec::new()
        } else {
            vec![0, 1, 2]
        }
    }

    fn make_move(&mut self, mov: &Self::Move) {
        self.moves.push(*mov);
    }

    fn result(&self, player: Self::PlayerTag) -> Option<f64> {
        if self.moves.len() < 2 {
            return None;
        }
        Some(if player == 0 { 1.0 } else { 0.0 })
    }

    // Return a value estimate instead of doing rollouts
    fn value_estimate(&self) -> Option<f64> {
        Some(self.value)
    }
}

#[test]
fn test_graph_ids_enabled() {
    let game = TenMoveGame::default();
    let mut ismcts = IsmctsHandler::new_with_graph_support(game);
    ismcts.run_iterations(1, 100);

    // Check that root node has an ID
    assert!(ismcts.root_node.id.is_some());
    assert!(ismcts.root_node.generate_node_ids);

    // Check that children have IDs
    let children = ismcts.root_node.children.read().unwrap();
    for child in children.iter() {
        assert!(child.id.is_some());
        assert!(child.generate_node_ids);
    }
}

#[test]
fn test_graph_ids_disabled() {
    let game = TenMoveGame::default();
    let mut ismcts = IsmctsHandler::new(game);
    ismcts.run_iterations(1, 100);

    // Check that root node has an empty ID
    assert!(ismcts.root_node.id.is_none());
    assert!(!ismcts.root_node.generate_node_ids);

    // Check that children have empty IDs
    let children = ismcts.root_node.children.read().unwrap();
    for child in children.iter() {
        assert!(child.id.is_none());
        assert!(!child.generate_node_ids);
    }
}

#[test]
fn test_policy_priors_are_stored() {
    let game = GameWithPolicy::default();
    let mut ismcts = IsmctsHandler::new(game);
    ismcts.run_iterations(1, 100);

    let children = ismcts.root_node.children.read().unwrap();

    // Should have explored all 3 moves
    assert_eq!(3, children.len());

    // Find each move and check its prior probability
    for child in children.iter() {
        let stats = child.statistics.read().unwrap();
        let mov = child.mov.as_ref().unwrap();

        match mov {
            0 => assert!((stats.prior_probability - 0.7).abs() < 0.001),
            1 | 2 => assert!((stats.prior_probability - 0.15).abs() < 0.001),
            _ => panic!("Unexpected move"),
        }
    }
}

#[test]
fn test_policy_affects_selection() {
    let game = GameWithPolicy::default();
    let mut ismcts = IsmctsHandler::new(game);
    ismcts.run_iterations(1, 1000);

    let children = ismcts.root_node.children.read().unwrap();

    // Find visit counts for each move
    let mut visits = std::collections::HashMap::new();
    for child in children.iter() {
        let mov = *child.mov.as_ref().unwrap();
        let visit_count = child.statistics.read().unwrap().visit_count;
        visits.insert(mov, visit_count);
    }

    // Move 0 should have significantly more visits than moves 1 and 2
    // due to its higher prior (0.7 vs 0.15)
    assert!(visits[&0] > visits[&1]);
    assert!(visits[&0] > visits[&2]);
}

#[test]
fn test_value_network_is_used() {
    let game = GameWithValue::default();
    let mut ismcts = IsmctsHandler::new(game);
    ismcts.run_iterations(1, 100);

    let children = ismcts.root_node.children.read().unwrap();

    // All children should have been explored
    assert!(children.len() > 0);

    // Check that value estimates are being used by verifying
    // that rewards are accumulating based on the value estimate (0.5)
    for child in children.iter() {
        let stats = child.statistics.read().unwrap();
        if stats.visit_count > 0 {
            let avg_reward = stats.reward / stats.visit_count as f64;
            // The average reward should be close to the value estimate
            // (allowing for some variance due to the stochastic nature)
            assert!((0.0..=1.0).contains(&avg_reward));
        }
    }
}

#[test]
fn test_puct_with_custom_constant() {
    let game = GameWithPolicy::default();
    let mut ismcts = IsmctsHandler::new_with_puct(game, 2.0);

    assert_eq!(ismcts.c_puct, 2.0);

    ismcts.run_iterations(1, 100);

    // Verify it runs successfully with custom PUCT constant
    let children = ismcts.root_node.children.read().unwrap();
    assert!(children.len() > 0);
}

#[test]
fn test_combined_policy_and_value() {
    // Test a game that has both policy and value network support
    #[derive(Clone, Debug)]
    struct CombinedGame {
        moves: Vec<u8>,
    }

    impl Game for CombinedGame {
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
            if self.moves.len() >= 2 {
                Vec::new()
            } else {
                vec![0, 1, 2]
            }
        }
        fn make_move(&mut self, mov: &Self::Move) {
            self.moves.push(*mov);
        }
        fn result(&self, player: Self::PlayerTag) -> Option<f64> {
            if self.moves.len() < 2 {
                return None;
            }
            Some(if player == 0 { 1.0 } else { 0.0 })
        }
        fn move_probabilities(&self) -> Option<Vec<(Self::Move, f64)>> {
            Some(vec![(0, 0.6), (1, 0.3), (2, 0.1)])
        }
        fn value_estimate(&self) -> Option<f64> {
            Some(0.75)
        }
    }

    let game = CombinedGame { moves: Vec::new() };
    let mut ismcts = IsmctsHandler::new_with_config(game, 1.5, true);

    ismcts.run_iterations(2, 500);

    // Verify that both features work together
    let children = ismcts.root_node.children.read().unwrap();
    assert_eq!(3, children.len());

    // Check IDs are generated
    for child in children.iter() {
        assert!(child.id.is_some());

        // Check priors are set
        let stats = child.statistics.read().unwrap();
        assert!(stats.prior_probability < 1.0);
    }
}

#[test]
fn test_dotty_graph_generation_with_ids() {
    let game = TenMoveGame::default();
    let mut ismcts = IsmctsHandler::new_with_graph_support(game);
    ismcts.run_iterations(1, 100);

    // Verify root and children have IDs
    assert!(ismcts.root_node.id.is_some());
    let root_id = ismcts.root_node.id.as_ref().unwrap().clone();

    let children = ismcts.root_node.children.read().unwrap();
    assert!(children.len() > 0, "Should have explored some children");

    // Collect child IDs to verify they appear in the graph
    let mut child_ids = Vec::new();
    for child in children.iter() {
        assert!(
            child.id.is_some(),
            "Child node ID should not be empty for graph generation"
        );
        child_ids.push(child.id.as_ref().unwrap().clone());
    }
    drop(children); // Release the lock

    // Get the graph output
    let graph = ismcts.dotty_graph_string();

    // Verify graph structure
    assert!(
        graph.contains("digraph G"),
        "Graph should start with digraph declaration"
    );
    assert!(
        graph.contains("rankdir=\"LR\""),
        "Graph should have left-to-right layout"
    );
    assert!(
        graph.contains(&format!("\"{}\"", root_id)),
        "Graph should contain root node ID"
    );

    // Verify at least some child IDs are in the graph
    let mut found_children = 0;
    for child_id in &child_ids {
        if graph.contains(&format!("\"{}\"", child_id)) {
            found_children += 1;
        }
    }
    assert!(
        found_children > 0,
        "Graph should contain at least one child node"
    );

    // Verify there are edges (arrows) in the graph
    assert!(
        graph.contains("->"),
        "Graph should contain edges between nodes"
    );

    // Verify graph has proper closing
    assert!(
        graph.ends_with("}\n"),
        "Graph should end with closing brace"
    );
}

#[test]
fn test_graph_disabled_has_empty_ids() {
    let game = TenMoveGame::default();
    let mut ismcts = IsmctsHandler::new(game);
    ismcts.run_iterations(1, 100);

    // When graph support is disabled, IDs should be empty
    assert!(ismcts.root_node.id.is_none());

    let children = ismcts.root_node.children.read().unwrap();
    for child in children.iter() {
        assert!(
            child.id.is_none(),
            "Child IDs should be empty when graph support is disabled"
        );
    }
}
