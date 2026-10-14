//! Core data structures and types for the planning engine.
//!
//! The central type is [`PlanBranch`]: one hypothesis in the backward search. A branch is a
//! partial action chain plus the list of needs (open preconditions and open requirements) that
//! the chain does not yet satisfy. Each need is tagged with the chain position of its consumer,
//! so that when [`PlanBranch::insert_action_at`] splices a predecessor action into the chain,
//! every position behind it can be shifted consistently.
//!
//! [`SearchContext`] holds everything that is shared and read-mostly across all branches of one
//! planning run: the action specs, the initial snapshots, the channels to the main thread, and
//! the discovery caches.

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
    /// Backward phase: the branch still has open needs and is expanded by inserting actions.
    Searching,
    /// Forward phase: all needs are matched symbolically and the chain is being replayed from
    /// the initial state to confirm it really works (see
    /// [`PlannerEngine`](crate::planner::PlannerEngine)).
    Verifying,
}

/// A single branch in the planning search tree.
///
/// It tracks the sequence of actions, open needs, and the simulated state of the
/// agent and world after applying those actions.
///
/// The chain is built backward but stored in execution order: a predecessor is inserted
/// in front of the first action that consumes what it provides. Every `usize` position
/// stored next to a need or binding is an index into `action_chain` and means "the state
/// just before the action at that index runs". A position equal to the chain length
/// therefore refers to the final state, which is where goal preconditions live.
///
/// While [`BranchState::Searching`], `current_agent` and `current_world` are just the
/// initial snapshots; they only advance during [`BranchState::Verifying`].
#[derive(Clone, Debug)]
pub struct PlanBranch {
    /// Indices into the `SearchContext.actions` list.
    pub action_chain: Vec<usize>,
    /// Costs of each action in the chain, parallel to `action_chain`. Seeded from the
    /// discovery cost when an action is inserted; verification reuses these values rather
    /// than asking GDScript again.
    pub action_costs: Vec<f64>,
    /// Variable bindings associated with actions at specific chain positions: the concrete
    /// values that wildcard provisions (such as `at_target`) were matched with. Several
    /// entries can share a position.
    pub action_bindings: Vec<(usize, String, Vec<VariantSnapshot>)>,
    /// Preconditions that are not yet satisfied by preceding actions or the initial state.
    /// The position is the chain index of the consumer; each is checked in the simulated
    /// state just before that index.
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
    pub current_agent: BlackboardSnapshot,
    /// Simulated world state after `simulation_index` actions.
    pub current_world: BlackboardSnapshot,
    /// Total grounded cost of the actions in this branch: always the sum of `action_costs`
    /// (see [`PlanBranch::recalculate_cost`]).
    pub cost: f64,
    /// ID of the node in the debug search tree.
    pub tree_node_id: usize,
}

/// A node in the search queue.
///
/// Wraps a [`PlanBranch`] with the bookkeeping needed to park it while a GDScript callback is
/// outstanding and to resume it afterwards without redoing work.
#[derive(Clone, Debug)]
pub struct SearchNode {
    /// The hypothesis this node represents.
    pub branch: PlanBranch,
    /// True when the node was parked on a callback and has been re-enqueued with its answer.
    /// Resumed nodes skip the visited-set check, since their fingerprint was recorded when
    /// they were first popped.
    pub resumed: bool,
    /// The main thread's answer to the request this node was parked on, consumed by the next
    /// simulation step. Cleared once that step makes progress.
    pub callback_response: Option<CallbackResponse>,
    /// Candidates `(action_idx, bindings)` already expanded from this node in the current
    /// pass, so the same candidate is not turned into two children when discovery yields it
    /// twice. Cleared when the node resumes after a callback, since discovery then reruns in
    /// full.
    pub expanded_candidates: Vec<(usize, BindingMap)>,
}

/// Wrapper that pairs a [`SearchNode`] with its pre-computed priority so the
/// `BinaryHeap` ordering does not depend on the heuristic object.
///
/// The [`Ord`] implementation inverts the comparison so the heap behaves as a
/// min-heap (lowest priority expanded first).
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

/// Concrete values bound to a name for one action occurrence: `(binding_name, values)` pairs.
///
/// For a wildcard provision such as GoTo's `at_target` the entry is
/// `("at_target", <the consumer's fact args>)`, which is how one generic action gets pinned
/// to a concrete target.
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

/// A request for background discovery simulation or precondition check.
///
/// Recorded in [`SearchContext::discovery_request_map`] so that, when the main thread's
/// answer arrives, the engine knows which cache to fill.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DiscoveryRequest {
    /// Cost or effect simulation of an action: `(action_idx, bindings)`.
    Simulation(usize, BindingMap),
    /// Custom precondition or validity check: `(action_idx, spec, bindings)`.
    Precondition(usize, PreconditionSpec, BindingMap),
}

/// Shared context and cached data for a single planning run.
///
/// Wrapped in an `Arc` and shared by the engine and the expander. The immutable fields are
/// set up once by the scheduler; the `Mutex` maps are discovery caches that live for the whole
/// run and are shared by all branches, so a given `(action, bindings)` pair costs at most one
/// round trip to GDScript no matter how many branches need it. No lock is ever held while a
/// callback is being sent.
pub struct SearchContext {
    /// All candidate actions; indices into this vector are what `PlanBranch::action_chain`
    /// and `PlanResult::action_chain` contain.
    pub actions: Vec<ActionSpec>,
    /// Agent state at submission time; every branch simulates forward from here.
    pub initial_agent: BlackboardSnapshot,
    /// World state at submission time.
    pub initial_world: BlackboardSnapshot,
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

/// Extracts the binding name and value from a provision/requirement pair.
///
/// A `Binding` provision binds its own value. A `Fact` provision binds its own args. A
/// `FactWildcard` provision binds the args of the requirement it is matched against; this is
/// where a wildcard gets pinned to a concrete target. Returns an empty name when the pair
/// produces no binding.
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

    /// Returns a hash identifying this branch by its goal, open needs, state,
    /// and concrete binding values.
    ///
    /// Used for cycle detection and search space pruning. The hash includes
    /// `action_bindings` to prevent false collisions where two branches have
    /// identical open needs but different concrete binding values that affect
    /// future candidate discovery. The action chain itself is deliberately not hashed:
    /// two chains that leave the same needs open are treated as interchangeable, so only the
    /// cheaper one is kept (see the visited check in the engine). The positions of open needs are not
    /// hashed; the positions of bindings are.
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

    /// Increments the chain position of all open needs and bindings at or after
    /// `from` by `delta`. Called when an action is spliced in at `from` so that everything
    /// that used to point at a later action keeps pointing at it.
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
                // Sort indices for deterministic iteration order (HashSet is unordered).
                let mut sorted_indices: Vec<usize> = reqs_to_remove.iter().copied().collect();
                sorted_indices.sort_unstable();
                for idx in sorted_indices {
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
/// Returns the smallest position among the consumers it satisfies, so the new action runs
/// before all of them. Falls back to `0` when no consumer sits before the end of the chain
/// (for instance an empty chain).
///
/// `satisfied_precondition_indices` and `satisfied_requirements` index into the branch's
/// open lists as they are before insertion.
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
