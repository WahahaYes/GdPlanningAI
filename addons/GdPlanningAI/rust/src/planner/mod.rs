pub mod engine;
pub mod expander;
pub mod simulation;
pub mod types;

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
