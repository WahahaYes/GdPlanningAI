use crate::plan_types::*;
use crate::snapshot::{BlackboardSnapshot, VariantSnapshot};
use crate::requirement::{ProvisionSpec, RequirementSpec};
use std::sync::mpsc::Sender;
use std::collections::HashMap;

use std::cmp::Ordering;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BranchState {
    Initializing,
    Searching,
    Rippling,
    Verifying,
}

#[derive(Clone, Debug)]
pub struct PlanBranch {
    pub action_chain: Vec<usize>, // Indices into ctx.actions
    pub action_costs: Vec<f64>,   // Costs of actions in the chain (filled during simulation)
    pub action_bindings: Vec<(usize, String, Vec<VariantSnapshot>)>, // (chain_pos, fact_name, values)
    pub open_preconditions: Vec<(usize, PreconditionSpec)>, // (consumer_pos, spec)
    pub open_requirements: Vec<(usize, RequirementSpec)>, // (consumer_pos, spec)
    pub state: BranchState,
    pub goal_index: usize,
    pub simulation_index: usize,
    pub current_agent: BlackboardSnapshot,
    pub current_world: BlackboardSnapshot,
    pub cost: f64,
}

#[derive(Clone, Debug)]
pub struct SearchNode {
    pub branch: PlanBranch,
    pub resumed: bool,
    pub callback_response: Option<CallbackResponse>,
}

impl SearchNode {
    pub fn priority(&self) -> f64 {
        // A* priority: f(n) = g(n) + h(n)
        // g(n) is the accumulated cost
        // h(n) is the heuristic (number of open needs)
        let g = self.branch.cost;
        let h = (self.branch.open_preconditions.len() + self.branch.open_requirements.len()) as f64;
        g + h
    }
}

impl PartialEq for SearchNode {
    fn eq(&self, other: &Self) -> bool {
        self.priority() == other.priority()
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
        // BinaryHeap is a max-heap, so we invert the comparison for a min-heap
        other.priority().partial_cmp(&self.priority()).unwrap_or(Ordering::Equal)
    }
}

pub struct SearchContext {
    pub actions: Vec<ActionSpec>,
    pub initial_agent: BlackboardSnapshot,
    pub initial_world: BlackboardSnapshot,
    pub initial_provisions: Vec<ProvisionSpec>,
    pub request_tx: Sender<CallbackRequest>,
    pub engine_response_tx: Sender<PlannerCallback>,
    
    // Discovery Cache (Thread-safe)
    pub discovery_results: std::sync::Mutex<HashMap<usize, DiscoveryResult>>,
    pub discovery_pending: std::sync::Mutex<HashMap<usize, usize>>, // action_idx -> request_id
    pub discovery_request_map: std::sync::Mutex<HashMap<usize, usize>>, // request_id -> action_idx
}

#[derive(Clone)]
pub struct DiscoveryResult {
    pub agent: BlackboardSnapshot,
    pub world: BlackboardSnapshot,
    pub cost: f64,
}

impl PlanBranch {
    pub fn new(initial_agent: &BlackboardSnapshot, initial_world: &BlackboardSnapshot) -> Self {
        Self {
            action_chain: Vec::new(),
            action_costs: Vec::new(),
            action_bindings: Vec::new(),
            open_preconditions: Vec::new(),
            open_requirements: Vec::new(),
            state: BranchState::Initializing,
            goal_index: 0,
            simulation_index: 0,
            current_agent: initial_agent.clone(),
            current_world: initial_world.clone(),
            cost: 0.0,
        }
    }

    pub fn fingerprint(&self) -> (usize, Vec<(usize, PreconditionSpec)>, Vec<(usize, RequirementSpec)>, BranchState) {
        (self.goal_index, self.open_preconditions.clone(), self.open_requirements.clone(), self.state.clone())
    }
}
