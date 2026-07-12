//! Core data structures and types for the planning engine.

use crate::plan_types::*;
use crate::requirement::{ProvisionSpec, RequirementSpec};
use crate::snapshot::{BlackboardSnapshot, VariantSnapshot};
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::mpsc::Sender;

use std::cmp::Ordering;

/// The search state of a plan branch.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BranchState {
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

/// A node in the search queue.
#[derive(Clone, Debug)]
pub struct SearchNode {
    pub branch: PlanBranch,
    pub resumed: bool,
    pub callback_response: Option<CallbackResponse>,
    /// Candidates already expanded from this node. Prevents duplicate child
    /// creation when a node is resumed after pending callbacks complete.
    pub expanded_candidates: Vec<(usize, BindingMap)>,
}

/// Wrapper that pairs a [`SearchNode`] with its pre-computed priority so the
/// [`BinaryHeap`] ordering does not depend on the heuristic object.
///
/// The [`Ord`] implementation inverts the comparison so the heap behaves as a
/// min-heap (lowest priority expanded first).
#[derive(Clone, Debug)]
pub struct PriorityNode {
    pub priority: f64,
    pub node: SearchNode,
}

impl PartialEq for PriorityNode {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}

impl Eq for PriorityNode {}

impl PartialOrd for PriorityNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PriorityNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeap is a max-heap, so we invert for min-heap behaviour.
        other
            .priority
            .partial_cmp(&self.priority)
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

/// Removes elements at the given indices from a vector.
///
/// Indices are sorted descending so each removal does not affect
/// the validity of the remaining indices.
fn remove_indices<T>(vec: &mut Vec<T>, indices: &HashSet<usize>) {
    let mut sorted: Vec<_> = indices.iter().copied().collect();
    sorted.sort_unstable_by(|a, b| b.cmp(a));
    for idx in sorted {
        if idx < vec.len() {
            vec.remove(idx);
        }
    }
}

/// Extracts the binding name and value from a provision/requirement pair.
///
/// Returns an empty name when the provision does not produce a binding.
pub(crate) fn binding_values_from_match(
    prov: &ProvisionSpec,
    req: &RequirementSpec,
) -> (String, Vec<VariantSnapshot>) {
    match (prov, req) {
        (
            ProvisionSpec::Binding {
                binding_name,
                value,
            },
            _,
        ) => (binding_name.clone(), vec![value.clone()]),
        (ProvisionSpec::Fact { fact_name, args }, _) => (fact_name.clone(), args.clone()),
        (ProvisionSpec::FactWildcard { fact_name }, RequirementSpec::Fact { args, .. }) => {
            (fact_name.clone(), args.clone())
        }
        _ => (String::new(), vec![]),
    }
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
            state: BranchState::Searching,
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

    /// Returns all action bindings associated with the given chain position,
    /// collapsed into `(name, values)` pairs.
    pub fn collect_bindings_for_position(
        &self,
        position: usize,
    ) -> Vec<(String, Vec<VariantSnapshot>)> {
        self.action_bindings
            .iter()
            .filter(|(pos, _, _)| *pos == position)
            .map(|(_, name, vals)| (name.clone(), vals.clone()))
            .collect()
    }

    /// Increments the chain position of all open needs and bindings at or after
    /// `from` by `delta`.
    pub fn shift_positions(&mut self, from: usize, delta: usize) {
        for (pos, _) in self.open_preconditions.iter_mut() {
            if *pos >= from {
                *pos += delta;
            }
        }
        for (pos, _) in self.open_requirements.iter_mut() {
            if *pos >= from {
                *pos += delta;
            }
        }
        for (pos, _, _) in self.action_bindings.iter_mut() {
            if *pos >= from {
                *pos += delta;
            }
        }
    }

    /// Insert a predecessor action into the chain immediately before its consumers.
    ///
    /// * `insert_pos` is the position of the first consumer this action satisfies. The new action
    ///   is inserted at `insert_pos`, and all existing entries at or after `insert_pos` are shifted
    ///   forward by one.
    /// * `action_idx` is the index of the action in [`SearchContext::actions`].
    /// * `cost` is the discovered cost of the action.
    /// * `satisfied_requirements` lists the requirements this action satisfies, with their
    ///   positions in `open_requirements` **before** the shift and the provision that matched them.
    ///   The helper clears all open requirements whose spec is identical to each entry, regardless
    ///   of the provided index.
    /// * `satisfied_precondition_indices` lists the indices into `open_preconditions` **before** the
    ///   shift that this action satisfies.
    /// * `new_preconditions` and `new_requirements` are the action's own unsatisfied needs that
    ///   should be added at the new action's position, after the shift.
    ///
    /// Returns the new bindings that were created so the caller can log them or attach them to a
    /// debug tree. The branch is left in `Searching` state; the caller decides whether to start
    /// verification.
    #[allow(clippy::too_many_arguments)]
    pub fn insert_action_at(
        &mut self,
        insert_pos: usize,
        action_idx: usize,
        cost: f64,
        satisfied_requirements: Vec<(usize, RequirementSpec, ProvisionSpec)>,
        satisfied_precondition_indices: HashSet<usize>,
        new_preconditions: Vec<PreconditionSpec>,
        new_requirements: Vec<RequirementSpec>,
    ) -> Vec<(usize, String, Vec<VariantSnapshot>)> {
        // 1. Insert the action and cost.
        self.action_chain.insert(insert_pos, action_idx);
        self.action_costs.insert(insert_pos, cost);

        // 2. Shift all position-tracked entries at or after insert_pos.
        self.shift_positions(insert_pos, 1);

        // 3. Collect requirements to remove and build bindings.
        let mut reqs_to_remove: HashSet<usize> = HashSet::new();
        let mut new_bindings = Vec::new();

        for (_req_idx, req, prov) in satisfied_requirements {
            // Greedy clearing: all identical requirements are satisfied by this one action.
            for (idx, (_, other_req)) in self.open_requirements.iter().enumerate() {
                if other_req == &req {
                    reqs_to_remove.insert(idx);
                }
            }

            let (binding_name, values) = binding_values_from_match(&prov, &req);
            if !binding_name.is_empty() {
                // Provider binding: at the newly inserted action's position.
                new_bindings.push((insert_pos, binding_name.clone(), values.clone()));

                // Consumer bindings: at each shifted consumer position.
                for &idx in &reqs_to_remove {
                    let consumer_pos = self.open_requirements[idx].0;
                    new_bindings.push((consumer_pos, binding_name.clone(), values.clone()));
                }
            }
        }

        // 4. Remove satisfied needs.
        remove_indices(&mut self.open_requirements, &reqs_to_remove);
        remove_indices(
            &mut self.open_preconditions,
            &satisfied_precondition_indices,
        );

        // 5. Add the new action's own needs at the new action's position.
        for pre in new_preconditions {
            self.open_preconditions.push((insert_pos, pre));
        }
        for req in new_requirements {
            self.open_requirements.push((insert_pos, req));
        }

        // 6. Merge new bindings.
        self.action_bindings.extend(new_bindings.clone());

        new_bindings
    }
}

/// Computes the chain position where a predecessor action should be inserted.
///
/// Returns the smallest position among the consumers it satisfies, falling back to `0` when the
/// chain is empty.
pub(crate) fn compute_insert_pos(
    branch: &PlanBranch,
    satisfied_precondition_indices: &[usize],
    satisfied_requirements: &[(usize, RequirementSpec, ProvisionSpec)],
) -> usize {
    let old_chain_len = branch.action_chain.len();
    let mut insert_pos = old_chain_len;

    for &idx in satisfied_precondition_indices {
        let pos = branch.open_preconditions[idx].0;
        if pos < insert_pos {
            insert_pos = pos;
        }
    }
    for (idx, _, _) in satisfied_requirements {
        let pos = branch.open_requirements[*idx].0;
        if pos < insert_pos {
            insert_pos = pos;
        }
    }

    if insert_pos == old_chain_len {
        insert_pos = 0;
    }
    insert_pos
}
