pub mod types;
pub mod simulation;
pub mod expander;
pub mod engine;

pub use engine::PlannerEngine;
pub use types::{PlanBranch, SearchContext};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchAlgorithm {
    AStar,
    Dijkstra,
    DepthFirst,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminationStrategy {
    FirstComplete,
    BestCost,
}
