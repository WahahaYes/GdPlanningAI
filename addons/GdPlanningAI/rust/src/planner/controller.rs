use std::collections::{BinaryHeap, HashSet};
use super::types::PlanBranch;
use crate::snapshot::StableSnapshot;
use crate::plan_types::PreconditionSpec;
use crate::requirement::RequirementSpec;

#[derive(Debug, Clone)]
pub struct SearchNode {
    pub branch: PlanBranch,
    pub depth: usize,
    pub estimated_remaining: f64,
}

impl PartialEq for SearchNode {
    fn eq(&self, other: &Self) -> bool {
        (self.branch.cost + self.estimated_remaining) == (other.branch.cost + other.estimated_remaining)
    }
}

impl Eq for SearchNode {}

impl PartialOrd for SearchNode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SearchNode {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Reverse for min-heap
        let self_f = self.branch.cost + self.estimated_remaining;
        let other_f = other.branch.cost + other.estimated_remaining;
        other_f.partial_cmp(&self_f).unwrap_or(std::cmp::Ordering::Equal)
    }
}

#[derive(PartialEq, Eq, Hash)]
struct NodeFingerprint {
    agent_state: StableSnapshot,
    world_state: StableSnapshot,
    open_preconditions: Vec<PreconditionSpec>,
    open_requirements: Vec<(usize, RequirementSpec)>,
}

pub struct AStarController {
    heap: BinaryHeap<SearchNode>,
    visited: HashSet<NodeFingerprint>,
}

impl AStarController {
    pub fn new() -> Self {
        Self {
            heap: BinaryHeap::new(),
            visited: HashSet::new(),
        }
    }

    pub fn push(&mut self, node: SearchNode) {
        self.heap.push(node);
    }

    pub fn pop(&mut self) -> Option<SearchNode> {
        while let Some(node) = self.heap.pop() {
            let stable_agent = StableSnapshot::from_blackboard(&node.branch.final_state_agent);
            let stable_world = StableSnapshot::from_blackboard(&node.branch.final_state_world);
            
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
            return Some(node);
        }
        None
    }
}
