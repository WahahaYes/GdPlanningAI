//! Core data structures and types for the planning engine.

use crate::plan_types::*;
use crate::requirement::{ProvisionSpec, RequirementSpec};
use crate::snapshot::{BlackboardSnapshot, VariantSnapshot};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::mpsc::Sender;

use std::cmp::Ordering;

/// The search state of a plan branch.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BranchState {
    Initializing,
    Searching,
    Verifying,
}

/// A single branch in the planning search tree.
///
/// It tracks the sequence of actions, open needs, and the simulated state of the
/// agent and world after applying those actions.
#[derive(Clone, Debug)]
pub struct PlanBranch {
    /// Indices into the `SearchContext.actions` list.
    pub action_chain: Vec<usize>,
    /// Costs of each action in the chain.
    pub action_costs: Vec<f64>,
    /// Variable bindings associated with actions at specific chain positions.
    pub action_bindings: Vec<(usize, String, Vec<VariantSnapshot>)>,
    /// Preconditions that are not yet satisfied by preceding actions or the initial state.
    pub open_preconditions: Vec<(usize, PreconditionSpec)>,
    /// Symbolic requirements not yet satisfied by preceding actions.
    pub open_requirements: Vec<(usize, RequirementSpec)>,
    /// Current search phase for this branch.
    pub state: BranchState,
    /// The index of the goal this branch is trying to satisfy.
    pub goal_index: usize,
    /// Current position in the chain being simulated/verified.
    pub simulation_index: usize,
    /// Simulated agent state after `simulation_index` actions.
    pub current_agent: BlackboardSnapshot,
    /// Simulated world state after `simulation_index` actions.
    pub current_world: BlackboardSnapshot,
    /// Total grounded cost of the actions in this branch.
    pub cost: f64,
    /// ID of the node in the debug search tree.
    pub tree_node_id: usize,
}

/// A node in the A* priority queue.
#[derive(Clone, Debug)]
pub struct SearchNode {
    pub branch: PlanBranch,
    pub resumed: bool,
    pub callback_response: Option<CallbackResponse>,
    /// Candidates already expanded from this node. Prevents duplicate child
    /// creation when a node is resumed after pending callbacks complete.
    pub expanded_candidates: Vec<(usize, BindingMap)>,
}

impl SearchNode {
    /// Returns the priority value used for the search queue (lower is better).
    pub fn priority(&self) -> f64 {
        // Use Dijkstra (h=0) for guaranteed optimality in hybrid simulation.
        self.branch.cost
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
        other
            .priority()
            .partial_cmp(&self.priority())
            .unwrap_or(Ordering::Equal)
    }
}

pub type BindingMap = Vec<(String, Vec<VariantSnapshot>)>;

/// A unique identity for a plan branch used for cycle detection and search space pruning.
pub type SearchFingerprint = u64;

/// Classification of a provision's kind for indexing actions by what they provide.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ProvisionKind {
    Binding,
    Fact,
    FactWildcard,
}

/// A request for background discovery simulation or precondition check.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DiscoveryRequest {
    Simulation(usize, BindingMap), // action_idx, bindings
    Precondition(usize, PreconditionSpec, BindingMap), // action_idx, spec, bindings
}

/// Shared context and cached data for a single planning run.
pub struct SearchContext {
    pub actions: Vec<ActionSpec>,
    pub initial_agent: BlackboardSnapshot,
    pub initial_world: BlackboardSnapshot,
    pub initial_provisions: Vec<ProvisionSpec>,
    pub request_tx: Sender<CallbackRequest>,
    pub engine_response_tx: Sender<PlannerCallback>,

    // Discovery Cache (Thread-safe)
    pub discovery_results: std::sync::Mutex<HashMap<(usize, BindingMap), DiscoveryResult>>,
    pub discovery_costs: std::sync::Mutex<HashMap<(usize, BindingMap), f64>>,
    pub discovery_pending: std::sync::Mutex<HashMap<(usize, BindingMap), usize>>, // action_idx, bindings -> request_id
    pub discovery_request_map: std::sync::Mutex<HashMap<usize, DiscoveryRequest>>, // request_id -> DiscoveryRequest

    // Precondition Caching for Discovery (Initial State)
    pub discovery_precond_results:
        std::sync::Mutex<HashMap<(usize, PreconditionSpec, BindingMap), bool>>,
    pub discovery_precond_pending:
        std::sync::Mutex<HashMap<(usize, PreconditionSpec, BindingMap), usize>>,

    // Provision Index: maps (ProvisionKind, name) -> action indices that provide it.
    pub provision_index: HashMap<(ProvisionKind, String), Vec<usize>>,
    /// Action indices that have no wildcard provisions (can be discovered with empty bindings).
    pub non_wildcard_actions: Vec<usize>,
}

/// The result of a background action discovery simulation.
#[derive(Clone)]
pub struct DiscoveryResult {
    pub agent: BlackboardSnapshot,
    pub world: BlackboardSnapshot,
    pub cost: f64,
}

impl PlanBranch {
    /// Creates a new, empty plan branch starting from the initial states.
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
            tree_node_id: 0,
        }
    }

    /// Returns a unique identity for this branch based on its goal, open needs, state,
    /// and concrete binding values.
    ///
    /// Used for cycle detection and search space pruning. The hash includes
    /// `action_bindings` to prevent false collisions where two branches have
    /// identical open needs but different concrete binding values that affect
    /// future candidate discovery.
    pub fn fingerprint(&self) -> SearchFingerprint {
        let mut hasher = DefaultHasher::new();
        self.goal_index.hash(&mut hasher);
        self.state.hash(&mut hasher);
        for (_, pre) in &self.open_preconditions {
            pre.hash(&mut hasher);
        }
        for (_, req) in &self.open_requirements {
            req.hash(&mut hasher);
        }
        for (pos, name, values) in &self.action_bindings {
            pos.hash(&mut hasher);
            name.hash(&mut hasher);
            values.hash(&mut hasher);
        }
        hasher.finish()
    }

    /// Updates the total cost of the branch based on the individual action costs.
    pub fn recalculate_cost(&mut self) {
        self.cost = self.action_costs.iter().sum();
    }
}
