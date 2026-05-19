use std::collections::{BinaryHeap, HashSet};
use super::types::PlanBranch;
use crate::snapshot::StableSnapshot;
use crate::plan_types::PreconditionSpec;
use crate::requirement::RequirementSpec;

/// Common node data for all search algorithms
#[derive(Debug, Clone)]
pub struct SearchNode {
    pub branch: PlanBranch,
    pub depth: usize,
    pub estimated_remaining: f64,
}

/// Trait for search controllers - allows swapping between DFS, Dijkstra, A*
pub trait SearchController {
    fn push(&mut self, node: SearchNode);
    fn pop(&mut self) -> Option<SearchNode>;
}

/// Search algorithm configuration
#[derive(Debug, Clone, Copy)]
pub enum SearchAlgorithm {
    DepthFirst,
    Dijkstra,
    AStar,
}

/// Termination strategy
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TerminationStrategy {
    FirstComplete,  // Return first valid plan found (DFS-like)
    BestCost,       // Continue searching for optimal plan (A*/Dijkstra-like)
}

/// A* node ordered by cost + heuristic
#[derive(Debug, Clone)]
struct AStarNode(SearchNode);

impl PartialEq for AStarNode {
    fn eq(&self, other: &Self) -> bool {
        let self_f = self.0.branch.cost + self.0.estimated_remaining;
        let other_f = other.0.branch.cost + other.0.estimated_remaining;
        self_f == other_f
    }
}

impl Eq for AStarNode {}

impl PartialOrd for AStarNode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AStarNode {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let self_f = self.0.branch.cost + self.0.estimated_remaining;
        let other_f = other.0.branch.cost + other.0.estimated_remaining;
        other_f.partial_cmp(&self_f).unwrap_or(std::cmp::Ordering::Equal)
    }
}

/// Dijkstra node ordered by cost only
#[derive(Debug, Clone)]
struct DijkstraNode(SearchNode);

impl PartialEq for DijkstraNode {
    fn eq(&self, other: &Self) -> bool {
        self.0.branch.cost == other.0.branch.cost
    }
}

impl Eq for DijkstraNode {}

impl PartialOrd for DijkstraNode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DijkstraNode {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.0.branch.cost.partial_cmp(&self.0.branch.cost).unwrap_or(std::cmp::Ordering::Equal)
    }
}

#[derive(PartialEq, Eq, Hash)]
struct NodeFingerprint {
    agent_state: StableSnapshot,
    world_state: StableSnapshot,
    open_preconditions: Vec<PreconditionSpec>,
    open_requirements: Vec<(usize, RequirementSpec)>,
}

/// DFS Controller - uses stack (LIFO), no heuristic
pub struct DFSController {
    stack: Vec<SearchNode>,
}

impl DFSController {
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
        }
    }
}

impl SearchController for DFSController {
    fn push(&mut self, node: SearchNode) {
        self.stack.push(node);
    }

    fn pop(&mut self) -> Option<SearchNode> {
        self.stack.pop()
    }
}

/// Dijkstra Controller - uses priority queue by cost only
pub struct DijkstraController {
    heap: BinaryHeap<DijkstraNode>,
    visited: HashSet<NodeFingerprint>,
}

impl DijkstraController {
    pub fn new() -> Self {
        Self {
            heap: BinaryHeap::new(),
            visited: HashSet::new(),
        }
    }
}

impl SearchController for DijkstraController {
    fn push(&mut self, node: SearchNode) {
        self.heap.push(DijkstraNode(node));
    }

    fn pop(&mut self) -> Option<SearchNode> {
        while let Some(DijkstraNode(node)) = self.heap.pop() {
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

/// A* Controller - uses priority queue by cost + heuristic
pub struct AStarController {
    heap: BinaryHeap<AStarNode>,
    visited: HashSet<NodeFingerprint>,
}

impl AStarController {
    pub fn new() -> Self {
        Self {
            heap: BinaryHeap::new(),
            visited: HashSet::new(),
        }
    }
}

impl SearchController for AStarController {
    fn push(&mut self, node: SearchNode) {
        self.heap.push(AStarNode(node));
    }

    fn pop(&mut self) -> Option<SearchNode> {
        while let Some(AStarNode(node)) = self.heap.pop() {
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

/// Factory function to create controller based on algorithm
pub fn create_controller(algorithm: SearchAlgorithm) -> Box<dyn SearchController> {
    match algorithm {
        SearchAlgorithm::DepthFirst => Box::new(DFSController::new()),
        SearchAlgorithm::Dijkstra => Box::new(DijkstraController::new()),
        SearchAlgorithm::AStar => Box::new(AStarController::new()),
    }
}
