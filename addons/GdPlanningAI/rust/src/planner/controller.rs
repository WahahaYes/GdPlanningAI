//! Search controller for the planner.
//!
//! Provides the [`SearchController`] trait and implementations like [`DfsController`]
//! to manage the search frontier.

use super::expander::PlanBranch;

/// A node in the search frontier.
pub struct SearchNode {
    pub branch: PlanBranch,
    pub depth: usize,
    pub estimated_remaining: f64,
    /// ID of the corresponding node in [`crate::debug_tree::TreeDump`].
    pub tree_node_id: usize,
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
        for node in successors.into_iter().rev() {
            self.stack.push(node);
        }
    }

    fn frontier_size(&self) -> usize {
        self.stack.len()
    }
}
