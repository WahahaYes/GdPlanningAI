//! GOAP search: backward from the goal's unmet preconditions, then forward verification.
//! Send-safe; GDScript work is requested from the main thread.
//!
//! - [`engine`]: search loop and state machine.
//! - [`expander`]: action discovery and candidate generation.
//! - [`simulation`]: GDScript effects, costs and preconditions.
//! - [`types`]: search data structures.
//!
//! Also holds the [`SearchHeuristic`] and [`TerminationStrategy`] the engine is configured with.

pub mod engine;
pub mod expander;
pub mod simulation;
pub mod types;

pub use engine::PlannerEngine;
pub use types::{PlanBranch, ProvisionKind, SearchContext};

use types::SearchNode;

/// Pluggable priority and pruning for the search. Priorities are computed once at enqueue, so
/// they must be a pure function of the node.
pub trait SearchHeuristic: Send + Sync {
    /// Compute the priority of a node for the open-queue ordering.
    /// Lower values are expanded first (min-heap semantics).
    fn compute_priority(&self, node: &SearchNode) -> f64;

    /// Returns true if a node with `node_priority` cannot beat `best_cost`. Under
    /// [`TerminationStrategy::BestCost`] the first such pop ends the search.
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
    /// Return the first plan that passes forward verification, even if a cheaper one exists.
    FirstComplete,
    /// Search until no queued node can beat the best plan; cost-optimal with [`DijkstraHeuristic`].
    /// The scheduler uses this.
    BestCost,
}
