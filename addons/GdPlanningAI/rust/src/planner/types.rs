//! Search data structures.
//!
//! [`PlanBranch`] is one backward-search hypothesis: a partial chain plus its open needs, each
//! tagged with its consumer's chain position so [`PlanBranch::insert_action_at`] can shift them.
//! [`SearchContext`] holds what all branches share: specs, initial snapshots, channels and caches.

use crate::plan_types::*;
use crate::requirement::{ProvisionSpec, RequirementSpec};
use crate::snapshot::{BlackboardSnapshot, VariantSnapshot};
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::mpsc::Sender;

use std::cmp::Ordering;

/// The search state of a plan branch.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BranchState {
    /// Backward phase: the branch still has open needs and is expanded by inserting actions.
    Searching,
    /// Forward phase: replaying the chain from the initial state to confirm it works.
    Verifying,
}

/// One hypothesis in the backward search: a partial chain plus its open needs.
///
/// The chain is stored in execution order. A position next to a need or binding is the state just
/// before the action at that index; `chain.len()` is the final state, where goal preconditions
/// sit. `current_agent`/`current_world` only advance during [`BranchState::Verifying`]; they are
/// shared, so cloning a branch while searching copies no state.
#[derive(Clone, Debug)]
pub struct PlanBranch {
    /// Indices into the `SearchContext.actions` list.
    pub action_chain: Vec<usize>,
    /// Cost of each action, parallel to `action_chain`; seeded at discovery and reused in
    /// verification.
    pub action_costs: Vec<f64>,
    /// Values each wildcard provision was matched with, by chain position.
    pub action_bindings: Vec<(usize, String, Vec<VariantSnapshot>)>,
    /// Unsatisfied preconditions, tagged with their consumer's chain position.
    pub open_preconditions: Vec<(usize, PreconditionSpec)>,
    /// Symbolic requirements not yet satisfied by preceding actions. Positions are the chain
    /// index of the consuming action.
    pub open_requirements: Vec<(usize, RequirementSpec)>,
    /// Current search phase for this branch.
    pub state: BranchState,
    /// The index of the goal this branch is trying to satisfy.
    pub goal_index: usize,
    /// Index of the next chain action to simulate while verifying; also the position whose
    /// open preconditions are being evaluated. Always 0 while searching.
    pub simulation_index: usize,
    /// Simulated agent state after `simulation_index` actions.
    pub current_agent: Arc<BlackboardSnapshot>,
    /// Simulated world state after `simulation_index` actions.
    pub current_world: Arc<BlackboardSnapshot>,
    /// Total grounded cost of the actions in this branch: always the sum of `action_costs`
    /// (see [`PlanBranch::recalculate_cost`]).
    pub cost: f64,
    /// ID of the node in the debug search tree.
    pub tree_node_id: usize,
}

/// A queued branch, plus what is needed to park it on a callback and resume it.
#[derive(Clone, Debug)]
pub struct SearchNode {
    /// The hypothesis this node represents.
    pub branch: PlanBranch,
    /// Set when re-queued with a callback answer; skips the visited check.
    pub resumed: bool,
    /// The main thread's answer to the request this node was parked on, consumed by the next
    /// simulation step. Cleared once that step makes progress.
    pub callback_response: Option<CallbackResponse>,
    /// Candidates already expanded from this node, so a repeat from discovery is ignored. Cleared
    /// on resume.
    pub expanded_candidates: Vec<(usize, BindingMap)>,
}

/// A [`SearchNode`] with its precomputed priority. `Ord` is inverted so `BinaryHeap` pops the
/// lowest first.
#[derive(Clone, Debug)]
pub struct PriorityNode {
    /// Value from [`SearchHeuristic::compute_priority`](crate::planner::SearchHeuristic::compute_priority);
    /// lower is expanded first.
    pub priority: f64,
    /// The queued node.
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

/// `(binding_name, values)` pairs for one action occurrence, e.g. GoTo's
/// `("at_target", <the consumer's fact args>)`.
pub type BindingMap = Vec<(String, Vec<VariantSnapshot>)>;

/// A unique identity for a plan branch used for cycle detection and search space pruning.
pub type SearchFingerprint = u64;

/// Classification of a provision's kind for indexing actions by what they provide.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ProvisionKind {
    /// A [`ProvisionSpec::Binding`], looked up by binding name.
    Binding,
    /// A [`ProvisionSpec::Fact`], looked up by fact name.
    Fact,
    /// A [`ProvisionSpec::FactWildcard`], looked up by fact name; matches any `Fact`
    /// requirement of that name.
    FactWildcard,
}

/// A pending discovery simulation or precondition check, so its answer fills the right cache.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DiscoveryRequest {
    /// Cost or effect simulation of an action: `(action_idx, bindings)`.
    Simulation(usize, BindingMap),
    /// Custom precondition or validity check: `(action_idx, spec, bindings)`.
    Precondition(usize, PreconditionSpec, BindingMap),
}

/// Read-mostly state shared by all branches of a run: specs, initial snapshots, channels, and
/// the discovery caches (so each `(action, bindings)` costs at most one GDScript round trip).
pub struct SearchContext {
    /// All candidate actions; indices into this vector are what `PlanBranch::action_chain`
    /// and `PlanResult::action_chain` contain.
    pub actions: Vec<ActionSpec>,
    /// Agent state at submission time; every branch simulates forward from here.
    pub initial_agent: Arc<BlackboardSnapshot>,
    /// World state at submission time.
    pub initial_world: Arc<BlackboardSnapshot>,
    /// Bindings and facts that already hold before any action runs; requirements they match
    /// never become open needs.
    pub initial_provisions: Vec<ProvisionSpec>,
    /// Channel on which the engine asks the main thread to run a GDScript callable.
    pub request_tx: Sender<CallbackRequest>,
    /// Sender handed to the main thread so it can answer on the engine's own receiver.
    pub engine_response_tx: Sender<PlannerCallback>,

    /// Result (post-state and cost) of simulating `(action_idx, bindings)` against the
    /// initial state.
    pub discovery_results: std::sync::Mutex<HashMap<(usize, BindingMap), DiscoveryResult>>,
    /// Costs already fetched for `(action_idx, bindings)`. Kept separately because the cost
    /// arrives one callback before the effect does.
    pub discovery_costs: std::sync::Mutex<HashMap<(usize, BindingMap), f64>>,
    /// In-flight discovery simulations: `(action_idx, bindings)` -> request id.
    pub discovery_pending: std::sync::Mutex<HashMap<(usize, BindingMap), usize>>,
    /// Request id -> what that request was for. An id missing from here means its answer has
    /// already been processed, which is how stale `*_pending` entries are detected.
    pub discovery_request_map: std::sync::Mutex<HashMap<usize, DiscoveryRequest>>,

    /// Cached outcomes of custom preconditions and validity checks evaluated during
    /// discovery, keyed by `(action_idx, spec, bindings)`.
    pub discovery_precond_results:
        std::sync::Mutex<HashMap<(usize, PreconditionSpec, BindingMap), bool>>,
    /// In-flight custom precondition evaluations: key -> request id.
    pub discovery_precond_pending:
        std::sync::Mutex<HashMap<(usize, PreconditionSpec, BindingMap), usize>>,

    /// Provision index: maps `(ProvisionKind, name)` to the actions that provide it, so
    /// candidate discovery only visits actions that can match an open requirement.
    pub provision_index: HashMap<(ProvisionKind, String), Vec<usize>>,
    /// Action indices that have no wildcard provisions (can be discovered with empty bindings).
    pub non_wildcard_actions: Vec<usize>,
}

/// The result of a background action discovery simulation.
#[derive(Clone)]
pub struct DiscoveryResult {
    /// Agent state after simulating the action once against the initial state.
    pub agent: BlackboardSnapshot,
    /// World state after simulating the action once against the initial state.
    pub world: BlackboardSnapshot,
    /// The action's cost in that same simulation.
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

/// Returns the binding a provision creates when matched with `req`: a `Binding`'s value, a
/// `Fact`'s args, or for a `FactWildcard` the requirement's args. Empty name if none.
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
    pub fn new(
        initial_agent: Arc<BlackboardSnapshot>,
        initial_world: Arc<BlackboardSnapshot>,
    ) -> Self {
        Self {
            action_chain: Vec::new(),
            action_costs: Vec::new(),
            action_bindings: Vec::new(),
            open_preconditions: Vec::new(),
            open_requirements: Vec::new(),
            state: BranchState::Searching,
            goal_index: 0,
            simulation_index: 0,
            current_agent: initial_agent,
            current_world: initial_world,
            cost: 0.0,
            tree_node_id: 0,
        }
    }

    /// Hashes the goal, open needs with their positions, state and binding values for pruning. The
    /// chain is not hashed, so two chains leaving the same needs open at the same positions are
    /// interchangeable and only the cheaper is kept. Positions matter: verification can reject
    /// one ordering of the same actions and accept another.
    pub fn fingerprint(&self) -> SearchFingerprint {
        let mut hasher = DefaultHasher::new();
        self.goal_index.hash(&mut hasher);
        self.state.hash(&mut hasher);
        self.open_preconditions.hash(&mut hasher);
        self.open_requirements.hash(&mut hasher);
        for (pos, name, values) in &self.action_bindings {
            pos.hash(&mut hasher);
            name.hash(&mut hasher);
            values.hash(&mut hasher);
        }
        hasher.finish()
    }

    /// Recomputes the total cost as the sum of `action_costs`. Recomputing (rather than
    /// accumulating) keeps the value correct when actions are inserted mid-chain.
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

    /// Adds `delta` to every need and binding position at or after `from`.
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

    /// Inserts `action_idx` at `insert_pos` (its first consumer), shifting later positions.
    /// Clears the satisfied needs (indices refer to the lists before the shift) and adds the
    /// action's own needs at its position. Returns the new bindings.
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
            // Matched indices are scoped per requirement so a later requirement's
            // consumer bindings don't inherit earlier requirements' consumers.
            let mut matched: Vec<usize> = Vec::new();
            for (idx, (_, other_req)) in self.open_requirements.iter().enumerate() {
                if other_req == &req {
                    matched.push(idx);
                }
            }
            matched.sort_unstable();
            reqs_to_remove.extend(matched.iter().copied());

            let (binding_name, values) = binding_values_from_match(&prov, &req);
            if !binding_name.is_empty() {
                // Provider binding: at the newly inserted action's position.
                new_bindings.push((insert_pos, binding_name.clone(), values.clone()));

                // Consumer bindings: at each shifted consumer position matched by
                // this requirement only.
                for idx in matched {
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

/// Returns the earliest position among the consumers a new action satisfies; the goal sits at
/// the end of the chain, so an action serving only the goal runs last. Indices refer to the open
/// lists before insertion.
pub(crate) fn compute_insert_pos(
    branch: &PlanBranch,
    satisfied_precondition_indices: &[usize],
    satisfied_requirements: &[(usize, RequirementSpec, ProvisionSpec)],
) -> usize {
    let mut insert_pos = branch.action_chain.len();

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

    insert_pos
}
