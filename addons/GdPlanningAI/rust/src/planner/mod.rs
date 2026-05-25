//! The planner module provides the core GOAP planning logic.
//! 
//! It is divided into several sub-modules:
//! - `engine`: The main search loop and state machine.
//! - `expander`: Action discovery and candidate generation.
//! - `simulation`: Interaction with GDScript for effects and preconditions.
//! - `types`: Core data structures used during search.

pub mod engine;
pub mod expander;
pub mod simulation;
pub mod types;

pub use engine::PlannerEngine;
pub use types::{PlanBranch, SearchContext};

/// The algorithm to use for exploring the search space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchAlgorithm {
    AStar,
    Dijkstra,
    DepthFirst,
}

/// Strategy for when to stop the search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminationStrategy {
    FirstComplete,
    BestCost,
}
