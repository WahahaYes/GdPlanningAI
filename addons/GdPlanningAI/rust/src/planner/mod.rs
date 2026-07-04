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
pub use types::{PlanBranch, ProvisionKind, SearchContext};

use types::SearchNode;

/// Trait for modular search heuristics used by the planner.
///
/// The engine delegates priority computation and pruning decisions to the
/// active heuristic, allowing future algorithms (A*, weighted A*, etc.)
/// to be plugged in without changing the search loop.
pub trait SearchHeuristic: Send + Sync {
    /// Compute the priority of a node for the open-queue ordering.
    /// Lower values are expanded first (min-heap semantics).
    fn compute_priority(&self, node: &SearchNode) -> f64;

    /// Returns true if a node with the given priority can be pruned
    /// because it can no longer improve upon `best_cost`.
    fn prune_threshold_met(&self, node_priority: f64, best_cost: f64) -> bool;

    /// Human-readable name for debugging.
    fn name(&self) -> &'static str;
}

/// Dijkstra / uniform-cost heuristic: priority is exact path cost (g).
/// Guarantees optimality when paired with `TerminationStrategy::BestCost`.
pub struct DijkstraHeuristic;

impl SearchHeuristic for DijkstraHeuristic {
    fn compute_priority(&self, node: &SearchNode) -> f64 {
        node.branch.cost
    }

    fn prune_threshold_met(&self, node_priority: f64, best_cost: f64) -> bool {
        node_priority >= best_cost
    }

    fn name(&self) -> &'static str {
        "Dijkstra"
    }
}

/// A* heuristic placeholder.
///
/// Currently falls back to Dijkstra behaviour (h = 0) until an admissible
/// heuristic for open-precondition / open-requirement estimation is
/// implemented.
pub struct AStarHeuristic;

impl SearchHeuristic for AStarHeuristic {
    fn compute_priority(&self, node: &SearchNode) -> f64 {
        // TODO: add admissible heuristic estimate of remaining cost.
        // For now, h = 0 so this behaves identically to Dijkstra.
        node.branch.cost
    }

    fn prune_threshold_met(&self, node_priority: f64, best_cost: f64) -> bool {
        node_priority >= best_cost
    }

    fn name(&self) -> &'static str {
        "A* (h=0 placeholder)"
    }
}

/// Strategy for when to stop the search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminationStrategy {
    FirstComplete,
    BestCost,
}
