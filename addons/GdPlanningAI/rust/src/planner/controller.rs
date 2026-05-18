//! Search controller for the planner.
//!
//! Provides the [`SearchController`] trait and implementations like [`DfsController`]
//! to manage the search frontier.

use super::expander::PlanBranch;
use crate::plan_types::PreconditionSpec;
use crate::requirement::RequirementSpec;

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};
use crate::snapshot::StableSnapshot;

/// A node in the search frontier.
pub struct SearchNode {
    pub branch: PlanBranch,
    pub depth: usize,
    pub estimated_remaining: f64,
    /// ID of the corresponding node in [`crate::debug_tree::TreeDump`].
    pub tree_node_id: usize,
}

impl SearchNode {
    /// Total estimated cost (f = g + h).
    pub fn f_score(&self) -> f64 {
        self.branch.estimated_cost + self.estimated_remaining
    }
}

impl PartialEq for SearchNode {
    fn eq(&self, other: &Self) -> bool {
        self.f_score() == other.f_score() && self.depth == other.depth
    }
}

impl Eq for SearchNode {}

impl PartialOrd for SearchNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SearchNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // Min-heap: reverse the comparison.
        // We prioritize lower f-score.
        // If f-scores are equal, we prioritize deeper nodes (DFS-like tie-breaking).
        other.f_score().partial_cmp(&self.f_score())
            .unwrap_or(Ordering::Equal)
            .then_with(|| self.depth.cmp(&other.depth))
    }
}

/// Strategy for managing the search frontier.
pub trait SearchController {
    /// Add the initial root node to the frontier.
    fn push_initial(&mut self, node: SearchNode);
    /// Retrieve the next node to expand.
    fn pop_next(&mut self) -> Option<SearchNode>;
    /// Add successor nodes generated from an expansion.
    fn push_successors(&mut self, successors: Vec<SearchNode>);
    /// Returns the number of nodes currently in the frontier.
    fn frontier_size(&self) -> usize;
}

/// Depth-first search implementation of [`SearchController`].
pub struct DfsController {
    stack: Vec<SearchNode>,
}

impl DfsController {
    /// Create a new DFS controller.
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }
}

impl Default for DfsController {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchController for DfsController {
    fn push_initial(&mut self, node: SearchNode) {
        self.stack.push(node);
    }

    fn pop_next(&mut self) -> Option<SearchNode> {
        self.stack.pop()
    }

    fn push_successors(&mut self, successors: Vec<SearchNode>) {
        // We push successors in reverse order so they are popped in the original candidate order.
        for node in successors.into_iter().rev() {
            self.stack.push(node);
        }
    }

    fn frontier_size(&self) -> usize {
        self.stack.len()
    }
}

#[derive(PartialEq, Eq, Hash)]
struct NodeFingerprint {
    agent_state: StableSnapshot,
    world_state: StableSnapshot,
    open_preconditions: Vec<PreconditionSpec>,
    open_requirements: Vec<RequirementSpec>,
}

/// A* search implementation of [`SearchController`].
pub struct AStarController {
    heap: BinaryHeap<SearchNode>,
    visited: HashSet<NodeFingerprint>,
}

impl AStarController {
    /// Create a new A* controller.
    pub fn new() -> Self {
        Self {
            heap: BinaryHeap::new(),
            visited: HashSet::new(),
        }
    }
}

impl Default for AStarController {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchController for AStarController {
    fn push_initial(&mut self, node: SearchNode) {
        let stable_agent = StableSnapshot::from_blackboard(node.branch.accumulated_agent());
        let stable_world = StableSnapshot::from_blackboard(node.branch.accumulated_world());
        let fingerprint = NodeFingerprint {
            agent_state: stable_agent,
            world_state: stable_world,
            open_preconditions: node.branch.open_preconditions.clone(),
            open_requirements: node.branch.open_requirements.clone(),
        };
        self.visited.insert(fingerprint);
        self.heap.push(node);
    }

    fn pop_next(&mut self) -> Option<SearchNode> {
        self.heap.pop()
    }

    fn push_successors(&mut self, successors: Vec<SearchNode>) {
        for node in successors {
            let stable_agent = StableSnapshot::from_blackboard(node.branch.accumulated_agent());
            let stable_world = StableSnapshot::from_blackboard(node.branch.accumulated_world());
            let fingerprint = NodeFingerprint {
                agent_state: stable_agent,
                world_state: stable_world,
                open_preconditions: node.branch.open_preconditions.clone(),
                open_requirements: node.branch.open_requirements.clone(),
            };
            if self.visited.contains(&fingerprint) {
                continue;
            }
            self.visited.insert(fingerprint);
            self.heap.push(node);
        }
    }

    fn frontier_size(&self) -> usize {
        self.heap.len()
    }
}

/// Dijkstra search implementation of [`SearchController`].
/// Identical to A* but assumes h=0.
pub struct DijkstraController {
    heap: BinaryHeap<SearchNode>,
}

impl DijkstraController {
    /// Create a new Dijkstra controller.
    pub fn new() -> Self {
        Self { heap: BinaryHeap::new() }
    }
}

impl Default for DijkstraController {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchController for DijkstraController {
    fn push_initial(&mut self, mut node: SearchNode) {
        node.estimated_remaining = 0.0;
        self.heap.push(node);
    }

    fn pop_next(&mut self) -> Option<SearchNode> {
        self.heap.pop()
    }

    fn push_successors(&mut self, successors: Vec<SearchNode>) {
        for mut node in successors {
            node.estimated_remaining = 0.0;
            self.heap.push(node);
        }
    }

    fn frontier_size(&self) -> usize {
        self.heap.len()
    }
}
