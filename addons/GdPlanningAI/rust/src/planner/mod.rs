//! The planner module provides the core GOAP planning logic.
//!
//! The planner searches backward from a goal: unmet goal preconditions become open needs, and
//! each search step inserts an action that satisfies some of them (see [`engine`]). Once a
//! chain has no open needs it is verified by simulating it forward from the initial state.
//! Everything here is Send-safe and runs on a worker thread; anything that needs GDScript is
//! requested from the main thread through channels (see [`simulation`]).
//!
//! It is divided into several sub-modules:
//! - `engine`: The main search loop and state machine.
//! - `expander`: Action discovery and candidate generation.
//! - `simulation`: Interaction with GDScript for effects and preconditions.
//! - `types`: Core data structures used during search.
//!
//! This module itself holds the pluggable [`SearchHeuristic`] and the
//! [`TerminationStrategy`] that the engine is configured with.

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
///
/// Priorities are computed once, when a node is enqueued, and stored next to the node (see
/// [`types::PriorityNode`]), so implementations must be a pure function of the node.
pub trait SearchHeuristic: Send + Sync {
    /// Compute the priority of a node for the open-queue ordering.
    /// Lower values are expanded first (min-heap semantics).
    fn compute_priority(&self, node: &SearchNode) -> f64;

    /// Returns true if a node with the given priority can be pruned
    /// because it can no longer improve upon `best_cost`.
    ///
    /// The engine only uses this to stop the search under
    /// [`TerminationStrategy::BestCost`]: because the queue is ordered by priority, the first
    /// node that meets the threshold means every remaining node does too.
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
    /// Keep searching until the cheapest queued node cannot beat the best plan found so far.
    /// With [`DijkstraHeuristic`] the returned plan is cost-optimal. This is what the scheduler
    /// uses.
    BestCost,
}
