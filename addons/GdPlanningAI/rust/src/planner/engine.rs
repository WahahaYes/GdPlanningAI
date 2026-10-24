//! Backward-chaining Dijkstra search over [`PlanBranch`]es.
//!
//! A [`BranchState::Searching`] branch has open needs; [`find_candidates`] proposes actions that
//! satisfy some, each becoming a child branch. A branch with no open needs turns
//! [`BranchState::Verifying`]: it is replayed forward from the initial state, and the cheapest
//! chain that survives wins.
//!
//! [`PlannerEngine::plan`] never blocks. It returns `Pending(id)` while waiting on a main-thread
//! callback (the node is parked until the answer arrives) and `Pending(0)` when its budget runs
//! out; call it again either way.

use crate::debug_tree::{NodeOutcome, TreeDump};
use crate::plan_tree::PlanResult;
use crate::plan_types::*;
use crate::planner::simulation::{SimArgs, StepResult, eval_precondition, simulate_action};
use crate::planner::types::*;
use crate::requirement::{
    ProvisionSpec, RequirementSpec, provision_satisfies_requirement, requirement_holds_in_state,
};
use crate::snapshot::{BlackboardSnapshot, VariantSnapshot};

use super::{SearchHeuristic, TerminationStrategy};
use crate::planner::expander::find_candidates;
use std::collections::{BinaryHeap, HashMap};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, Sender};

/// The GOAP search engine. Goals are tried in the given order (highest reward first); the first
/// goal with a plan wins.
pub struct PlannerEngine {
    /// Shared per-run context: actions, initial state, channels and discovery caches.
    pub ctx: Arc<SearchContext>,
    /// Maximum chain length. Also bounds the number of open requirements as a cycle guard.
    pub max_depth: usize,
    /// Set from the main thread to abort the search; checked at several points per step.
    pub cancel_flag: Arc<AtomicBool>,
    /// Receiving end for the main thread's callback answers. The scheduler replaces this (and
    /// `response_tx`) with the channel whose sender it gave to [`SearchContext`].
    pub response_rx: Receiver<PlannerCallback>,
    /// Sending end matching `response_rx`.
    pub response_tx: Sender<PlannerCallback>,

    /// Orders the queue and decides when the search can stop early.
    pub heuristic: Box<dyn SearchHeuristic + Send + Sync>,
    /// When the search stops: first verified plan or best cost.
    pub termination: TerminationStrategy,
    /// Maximum node pops per [`PlannerEngine::plan`] call before yielding `Pending(0)`.
    pub iteration_budget: usize,
    /// Wall-clock budget per [`PlannerEngine::plan`] call before yielding `Pending(0)`;
    /// 0 disables the check.
    pub time_slice_ms: u64,
    /// Clock read by the time-slice check; [`std::time::Instant::now`] unless a test swaps it.
    pub clock: fn() -> std::time::Instant,

    /// Open nodes ordered by heuristic priority (lowest first).
    pub queue: BinaryHeap<PriorityNode>,
    /// Nodes waiting for a callback, keyed by the request id they wait on. Several nodes can
    /// wait on the same id (they share a discovery request).
    pub parked_nodes: HashMap<usize, Vec<SearchNode>>,
    /// Cheapest cost at which each searching-branch fingerprint has been expanded; see
    /// [`PlanBranch::fingerprint`].
    pub visited: HashMap<SearchFingerprint, f64>,
    /// Cheapest verified plan for the current goal so far.
    pub best_plan: Option<PlanResult>,
    /// Cost of `best_plan`; infinity until a plan is found. Branches at or above it are pruned.
    pub best_cost: f64,
    /// Position in the `goals` slice of the goal currently being searched.
    pub current_goal_index: usize,
    /// Debug record of the search; only populated when the log level was Debug at creation.
    pub tree: TreeDump,
    /// Post-states of verified steps, so branches sharing a chain prefix replay it without
    /// callbacks; see [`StepKey`]. Holds every distinct verified prefix until the job ends.
    pub step_cache: HashMap<StepKey, SharedState>,
    /// In-flight effect requests: key -> request id, so other branches wait on the same one.
    pub step_pending: HashMap<StepKey, usize>,
    /// Request id -> the step it simulates, so its answer fills `step_cache`.
    pub step_requests: HashMap<usize, StepKey>,
}

/// Agent and world state shared between branches.
pub type SharedState = (Arc<BlackboardSnapshot>, Arc<BlackboardSnapshot>);

/// One simulation step: the pre-state, the action and its bindings. Effects are deterministic, so
/// equal keys give equal post-states, and a branch replaying a cached state keys its next step
/// the same way.
pub type StepKey = (StateRef, StateRef, usize, BindingMap);

/// A shared snapshot compared and hashed by identity. Holding the `Arc` keeps the address
/// from being reused while a key refers to it.
#[derive(Clone, Debug)]
pub struct StateRef(pub Arc<BlackboardSnapshot>);

impl PartialEq for StateRef {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for StateRef {}

impl std::hash::Hash for StateRef {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}

impl PlannerEngine {
    /// Creates an engine (Dijkstra, [`TerminationStrategy::BestCost`], 20000 iterations, 10 ms).
    /// It owns a fresh response channel: replace `response_rx` if `ctx.engine_response_tx` was
    /// handed to the main thread.
    pub fn new(ctx: Arc<SearchContext>, max_depth: usize, cancel_flag: Arc<AtomicBool>) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Self {
            ctx,
            max_depth,
            cancel_flag,
            response_rx: rx,
            response_tx: tx,
            heuristic: Box::new(super::DijkstraHeuristic),
            termination: TerminationStrategy::BestCost,
            iteration_budget: 20000,
            time_slice_ms: 10,
            clock: std::time::Instant::now,
            queue: BinaryHeap::new(),
            parked_nodes: HashMap::new(),
            visited: HashMap::new(),
            best_plan: None,
            best_cost: f64::INFINITY,
            current_goal_index: 0,
            tree: TreeDump::new(),
            step_cache: HashMap::new(),
            step_pending: HashMap::new(),
            step_requests: HashMap::new(),
        }
    }

    /// Sets the search heuristic to use (e.g., Dijkstra, A*).
    pub fn with_heuristic(mut self, heuristic: Box<dyn SearchHeuristic + Send + Sync>) -> Self {
        self.heuristic = heuristic;
        self
    }

    /// Queues `node` with its priority computed once; its cost must not change afterwards.
    fn enqueue(&mut self, node: SearchNode) {
        let priority = self.heuristic.compute_priority(&node);
        self.queue.push(PriorityNode { priority, node });
    }
    /// Sets the termination strategy (e.g., FirstComplete, BestCost).
    pub fn with_termination_strategy(mut self, strat: TerminationStrategy) -> Self {
        self.termination = strat;
        self
    }
    /// Sets how many node pops a step may take before yielding `Pending(0)`.
    pub fn with_iteration_budget(mut self, budget: usize) -> Self {
        self.iteration_budget = budget;
        self
    }
    /// Sets the wall-clock budget per step (checked every 128 pops); 0 disables it.
    pub fn with_time_slice_ms(mut self, ms: u64) -> Self {
        self.time_slice_ms = ms;
        self
    }
    /// Sets the clock the time-slice check reads, so tests can make slices expire on demand.
    pub fn with_clock(mut self, clock: fn() -> std::time::Instant) -> Self {
        self.clock = clock;
        self
    }

    /// Runs one non-blocking slice of the search. Call again with the same `goals` until it returns
    /// [`PlannerRunResult::Complete`]: `None` means cancelled, a failed [`PlanResult`] means no plan.
    pub fn plan(&mut self, goals: &[GoalSpec]) -> PlannerRunResult {
        if self.queue.is_empty() && self.parked_nodes.is_empty() && self.best_plan.is_none() {
            // Initializing with the first goal
            if self.current_goal_index < goals.len() {
                self.initialize_goal(goals, self.current_goal_index);
            }
        }

        self.step_search(goals)
    }

    /// Logs the Info-level one-line plan summary: goal, outcome, and search cost.
    /// Always emitted (even at the default level) so runs stay observable.
    /// Per-node search detail lives on the tree; fetch it via `get_debug_tree()`.
    fn log_result_summary(&self, plan: &PlanResult, goals: &[GoalSpec]) {
        let goal_name = goals
            .iter()
            .find(|g| g.original_index as i64 == plan.goal_index)
            .map(|g| g.name.as_str())
            .unwrap_or("none");
        log_info!(
            "RESULT goal='{}' success={} actions={} cost={:.1} {}",
            goal_name,
            plan.success,
            plan.action_chain.len(),
            plan.total_cost,
            self.tree.summary()
        );
    }

    /// Seeds the queue with an empty-chain root whose open preconditions are the goal's unmet
    /// ones (custom preconditions always count as unmet).
    fn initialize_goal(&mut self, goals: &[GoalSpec], idx: usize) {
        let goal = &goals[idx];

        // Tree: Start goal
        let goal_pre_strings: Vec<String> =
            goal.desired_state.iter().map(|p| p.to_string()).collect();
        self.tree
            .begin_goal(&goal.name, goal.reward, &goal_pre_strings);

        let mut branch = PlanBranch::new(
            Arc::clone(&self.ctx.initial_agent),
            Arc::clone(&self.ctx.initial_world),
        );
        branch.goal_index = goal.original_index; // Use original index for Godot
        for pre in &goal.desired_state {
            let satisfied_by_initial = pre
                .evaluate_builtin(&self.ctx.initial_agent, &self.ctx.initial_world)
                .unwrap_or(false);
            if !satisfied_by_initial {
                branch.open_preconditions.push((0, pre.clone()));
            }
        }

        // Tree: Add root node
        let open_pre: Vec<String> = branch
            .open_preconditions
            .iter()
            .map(|(_, p)| p.to_string())
            .collect();
        let open_req: Vec<String> = branch
            .open_requirements
            .iter()
            .map(|(_, r)| r.to_string())
            .collect();
        branch.tree_node_id = self.tree.add_root(&open_pre, &open_req);

        self.enqueue(SearchNode {
            branch,
            resumed: false,
            callback_response: None,
            expanded_candidates: Vec::new(),
        });
    }

    /// One resumable slice: drain callback answers, pop nodes until the queue empties or a budget
    /// runs out, then wait for callbacks, move to the next goal, or finish.
    fn step_search(&mut self, goals: &[GoalSpec]) -> PlannerRunResult {
        if self.cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
            return PlannerRunResult::Complete(None);
        }

        // 1. Resume Callbacks
        while let Ok(callback) = self.response_rx.try_recv() {
            if self.cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                self.parked_nodes.clear();
                return PlannerRunResult::Complete(None);
            }

            // Answers to discovery requests (candidate discovery, as opposed to verification
            // steps of one branch) are written to the shared caches, so every node that needed
            // them can proceed, not just the one that triggered the request.
            let discovery_req = {
                let req_map = self.ctx.discovery_request_map.lock().unwrap();
                req_map.get(&callback.request_id).cloned()
            };

            if let Some(req) = discovery_req {
                match req {
                    DiscoveryRequest::Simulation(idx, bindings) => {
                        if let CallbackResponse::UpdatedSnapshots(ref agent, ref world) =
                            callback.response
                        {
                            let mut cache = self.ctx.discovery_results.lock().unwrap();
                            let cost = {
                                let costs = self.ctx.discovery_costs.lock().unwrap();
                                costs.get(&(idx, bindings.clone())).cloned().unwrap_or(1.0)
                            };
                            cache.insert(
                                (idx, bindings.clone()),
                                DiscoveryResult {
                                    agent: agent.clone(),
                                    world: world.clone(),
                                    cost,
                                },
                            );
                        } else if let CallbackResponse::Float(cost) = callback.response {
                            let mut costs = self.ctx.discovery_costs.lock().unwrap();
                            costs.insert((idx, bindings.clone()), cost);
                        }

                        let mut pending = self.ctx.discovery_pending.lock().unwrap();
                        pending.remove(&(idx, bindings));
                    }
                    DiscoveryRequest::Precondition(idx, spec, bindings) => {
                        if let CallbackResponse::Bool(b) = callback.response {
                            let mut cache = self.ctx.discovery_precond_results.lock().unwrap();
                            cache.insert((idx, spec.clone(), bindings.clone()), b);
                        }
                        let mut pending = self.ctx.discovery_precond_pending.lock().unwrap();
                        pending.remove(&(idx, spec, bindings));
                    }
                }
                let mut req_map = self.ctx.discovery_request_map.lock().unwrap();
                req_map.remove(&callback.request_id);
            }

            if let Some(key) = self.step_requests.remove(&callback.request_id) {
                self.step_pending.remove(&key);
                if let CallbackResponse::UpdatedSnapshots(ref agent, ref world) = callback.response
                {
                    self.step_cache
                        .entry(key)
                        .or_insert_with(|| (Arc::new(agent.clone()), Arc::new(world.clone())));
                }
            }

            // Nodes parked on this id resume with the answer attached. Discovery-parked nodes
            // ignore it and re-run discovery, which now hits the cache.
            if let Some(nodes) = self.parked_nodes.remove(&callback.request_id) {
                for mut node in nodes {
                    node.resumed = true;
                    node.callback_response = Some(callback.response.clone());
                    self.enqueue(node);
                }
            }
        }

        // 2. Main Search Loop
        let mut iterations = 0;
        let search_start = (self.clock)();
        let time_slice = std::time::Duration::from_millis(self.time_slice_ms);
        while let Some(priority_node) = self.queue.pop() {
            let node_priority = priority_node.priority;
            let mut node = priority_node.node;
            iterations += 1;

            // Optimality check: the queue pops the lowest priority first, so if even this
            // node's priority (g + h) cannot beat the best plan, nothing left in the queue can,
            // and under BestCost the search is finished.
            if self.termination == TerminationStrategy::BestCost
                && self
                    .heuristic
                    .prune_threshold_met(node_priority, self.best_cost)
            {
                self.enqueue(node); // Put it back for next time if needed
                break;
            }

            // Budget exhausted: put the node back and let the scheduler resume us later.
            if iterations > self.iteration_budget {
                log_warn!(
                    "Search budget exceeded ({} iterations). Search is taking too long.",
                    self.iteration_budget
                );
                self.enqueue(node);
                return PlannerRunResult::Pending(0);
            }

            if self.cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                return PlannerRunResult::Complete(None);
            }

            // Visited check (expansion-time dedup): a branch with the same fingerprint that was
            // already expanded at equal or lower cost makes this one redundant. Resumed nodes
            // were already recorded when first popped.
            if !node.resumed && node.branch.state == BranchState::Searching {
                let fp = node.branch.fingerprint();
                if let Some(&prev_cost) = self.visited.get(&fp)
                    && node.branch.cost >= prev_cost
                {
                    continue;
                }
                self.visited.insert(fp, node.branch.cost);
            }
            // A resumed node re-runs discovery from scratch; forgetting its expanded candidates
            // is safe because the children created before parking are already queued, and
            // the visited check then discards the duplicates.
            if node.resumed {
                node.expanded_candidates.clear();
            }
            node.resumed = false;

            // Prune anything that already costs at least as much as the best plan found.
            if node.branch.cost >= self.best_cost {
                continue;
            }

            // Time-sliced yield: same re-enqueue semantics as the
            // iteration-budget path. Checked every 128 pops to amortize
            // the clock read. Best-effort: the slice may be exceeded
            // between checks. Placed after prune checks so prunable
            // nodes don't cost an extra resume.
            if self.time_slice_ms > 0
                && iterations % 128 == 0
                && (self.clock)() - search_start >= time_slice
            {
                self.enqueue(node);
                return PlannerRunResult::Pending(0);
            }

            match node.branch.state {
                BranchState::Verifying => {
                    let proc_res = self.process_simulation(&mut node);
                    match proc_res {
                        StepResult::Ready(_) => self.enqueue(node),
                        StepResult::Pending(id) => {
                            self.parked_nodes.entry(id).or_default().push(node);
                        }
                        StepResult::Invalid => {
                            self.tree.set_outcome(
                                node.branch.tree_node_id,
                                NodeOutcome::Pruned {
                                    reason: "Simulation failed or cost infinite".to_string(),
                                },
                            );
                        }
                        StepResult::Complete => {
                            // Verification pass finished and updated best_plan.
                            // If strategy is FirstComplete, we are done.
                            if self.termination == TerminationStrategy::FirstComplete
                                && let Some(ref plan) = self.best_plan
                            {
                                if plan.action_chain.is_empty()
                                    && self.current_goal_index + 1 < goals.len()
                                {
                                    // Goal already satisfied (empty plan) but more goals remain.
                                    // Don't return yet; let step_search move to the next goal.
                                } else {
                                    let plan = self.best_plan.take().unwrap();
                                    self.log_result_summary(&plan, goals);
                                    return PlannerRunResult::Complete(Some(plan));
                                }
                            }
                            // Otherwise (BestCost), we just discard this branch and keep searching.
                        }
                    }
                }
                BranchState::Searching => {
                    // No open needs left: the chain is symbolically complete.
                    if node.branch.open_preconditions.is_empty()
                        && node.branch.open_requirements.is_empty()
                    {
                        self.begin_verification(&mut node.branch, &goals[self.current_goal_index]);
                        self.enqueue(node);
                        continue;
                    }

                    // Expand: depth-limited, so a branch at max_depth is a dead end.
                    if node.branch.action_chain.len() >= self.max_depth {
                        continue;
                    }
                    let candidates_res = find_candidates(&node.branch, &self.ctx);

                    for cand in candidates_res.ready {
                        let key = (cand.action_idx, cand.bindings.clone());
                        if node.expanded_candidates.contains(&key) {
                            continue;
                        }
                        node.expanded_candidates.push(key);

                        let mut new_branch = node.branch.clone();
                        let action = &self.ctx.actions[cand.action_idx];

                        // Record satisfied needs for debugging
                        let satisfied_pre: Vec<String> = cand
                            .satisfied_preconditions
                            .iter()
                            .map(|&idx| node.branch.open_preconditions[idx].1.to_string())
                            .collect();
                        let satisfied_req: Vec<String> = cand
                            .satisfied_requirements
                            .iter()
                            .map(|(_, req, _)| req.to_string())
                            .collect();

                        // Determine the insertion point: the earliest consumer position
                        // this candidate satisfies. The predecessor is placed immediately
                        // before that consumer, not at the front of the whole chain.
                        let insert_pos = compute_insert_pos(
                            &new_branch,
                            &cand.satisfied_preconditions,
                            &cand.satisfied_requirements,
                        );

                        let discovery_cost = {
                            let cache = self.ctx.discovery_results.lock().unwrap();
                            cache
                                .get(&(cand.action_idx, cand.bindings.clone()))
                                .map(|r| r.cost)
                                .unwrap_or(1.0)
                        };

                        // Collect the new action's own needs, deduplicating against the
                        // branch's current open needs and skipping anything already satisfied
                        // by the initial state.
                        let new_preconditions: Vec<PreconditionSpec> = action
                            .preconditions
                            .iter()
                            .filter(|pre| {
                                !new_branch.open_preconditions.iter().any(|(_, p)| p == *pre)
                                    && !pre
                                        .evaluate_builtin(
                                            &self.ctx.initial_agent,
                                            &self.ctx.initial_world,
                                        )
                                        .unwrap_or(false)
                            })
                            .cloned()
                            .collect();

                        let new_requirements: Vec<RequirementSpec> = action
                            .requirements
                            .iter()
                            .filter(|req| {
                                !new_branch.open_requirements.iter().any(|(_, r)| r == *req)
                                    && !self.ctx.initial_provisions.iter().any(|prov| {
                                        provision_satisfies_requirement(
                                            prov,
                                            req,
                                            Some(&self.ctx.initial_world),
                                        )
                                    })
                            })
                            .cloned()
                            .collect();

                        let _new_bindings = new_branch.insert_action_at(
                            insert_pos,
                            cand.action_idx,
                            discovery_cost,
                            cand.satisfied_requirements,
                            cand.satisfied_preconditions.iter().copied().collect(),
                            new_preconditions,
                            new_requirements,
                        );

                        // Tree: Add child node now that needs are updated
                        let open_pre: Vec<String> = new_branch
                            .open_preconditions
                            .iter()
                            .map(|(_, p)| p.to_string())
                            .collect();
                        let open_req: Vec<String> = new_branch
                            .open_requirements
                            .iter()
                            .map(|(_, r)| r.to_string())
                            .collect();

                        new_branch.tree_node_id = self.tree.add_child(
                            node.branch.tree_node_id,
                            crate::debug_tree::ChildNodeConfig {
                                action_name: &action.name,
                                estimated_cost: discovery_cost,
                                accumulated_cost: node.branch.cost + discovery_cost,
                                open_pre: &open_pre,
                                open_req: &open_req,
                                satisfied_pre: &satisfied_pre,
                                satisfied_req: &satisfied_req,
                            },
                        );

                        // Reset search state for the new branch
                        new_branch.state = BranchState::Searching;
                        new_branch.recalculate_cost();

                        // If ALL needs are satisfied, move to Verifying
                        if new_branch.open_preconditions.is_empty()
                            && new_branch.open_requirements.is_empty()
                        {
                            self.begin_verification(
                                &mut new_branch,
                                &goals[self.current_goal_index],
                            );
                        }

                        // Guard against runaway requirement accumulation (cycle detection).
                        // Legitimate chains can have at most max_depth open requirements.
                        if new_branch.open_requirements.len() > self.max_depth {
                            self.tree.set_outcome(
                                new_branch.tree_node_id,
                                NodeOutcome::Pruned {
                                    reason: "Too many accumulated open requirements (cycle)"
                                        .to_string(),
                                },
                            );
                            continue;
                        }

                        self.enqueue(SearchNode {
                            branch: new_branch,
                            resumed: false,
                            callback_response: None,
                            expanded_candidates: Vec::new(),
                        });
                    }

                    // Discovery was blocked on a callback for at least one action. Park the
                    // parent (its ready candidates were already expanded above) to retry once
                    // the answer lands.
                    if let Some(id) = candidates_res.pending_id {
                        node.callback_response = None;
                        self.parked_nodes.entry(id).or_default().push(node);
                    }
                }
            }
        }

        // The queue is empty. Outstanding callbacks mean we are waiting, not done. (Any parked
        // id works; the scheduler resumes us as soon as an answer for it has been delivered, and
        // an answer for another parked id is picked up on that resume too.)
        if !self.parked_nodes.is_empty() {
            PlannerRunResult::Pending(*self.parked_nodes.keys().next().unwrap())
        } else {
            // If the current goal is already satisfied (empty plan) and there are more goals,
            // skip to the next goal instead of returning the empty plan.
            if let Some(ref plan) = self.best_plan
                && plan.action_chain.is_empty()
                && self.current_goal_index + 1 < goals.len()
            {
                self.best_plan = None;
                self.best_cost = f64::INFINITY;
                self.tree.end_goal(false, &[], 0.0);
                self.current_goal_index += 1;
                self.initialize_goal(goals, self.current_goal_index);
                return PlannerRunResult::Pending(0);
            }

            // Search exhausted for current goal. Check if there are more goals.
            if self.best_plan.is_none() && self.current_goal_index + 1 < goals.len() {
                self.tree.end_goal(false, &[], 0.0);
                self.current_goal_index += 1;
                self.initialize_goal(goals, self.current_goal_index);
                // Return Pending(0) to signal we yielded to start the next goal
                return PlannerRunResult::Pending(0);
            }

            // Search exhausted or optimal plan found. With no plan, report failure so the caller
            // can distinguish it from cancellation (`Complete(None)`).
            let final_plan = self.best_plan.take().unwrap_or_else(PlanResult::failure);

            // With no goals there was nothing to search, so there is nothing to report.
            if !goals.is_empty() {
                if final_plan.success {
                    let action_names: Vec<String> = final_plan
                        .action_chain
                        .iter()
                        .map(|&idx| self.ctx.actions[idx as usize].name.clone())
                        .collect();
                    self.tree
                        .end_goal(true, &action_names, final_plan.total_cost);
                } else {
                    self.tree.end_goal(false, &[], 0.0);
                    log_debug!("no plan found; inspect the search via get_debug_tree()");
                }
                self.log_result_summary(&final_plan, goals);
            }
            PlannerRunResult::Complete(Some(final_plan))
        }
    }

    /// Restarts a symbolically complete branch from the initial state to verify it forward. A
    /// later insertion can undo a need an earlier one satisfied, so needs are re-opened where
    /// they are checked against the simulated state: each action's custom preconditions before it
    /// (builtin ones are re-checked by the action validator) and the goal after the last action.
    fn begin_verification(&self, branch: &mut PlanBranch, goal: &GoalSpec) {
        for (pos, &action_idx) in branch.action_chain.iter().enumerate() {
            let customs = self.ctx.actions[action_idx]
                .preconditions
                .iter()
                .filter(|pre| matches!(pre, PreconditionSpec::Custom { .. }));
            branch
                .open_preconditions
                .extend(customs.map(|pre| (pos, pre.clone())));
        }
        let end = branch.action_chain.len();
        branch
            .open_preconditions
            .extend(goal.desired_state.iter().map(|pre| (end, pre.clone())));
        branch.state = BranchState::Verifying;
        branch.simulation_index = 0;
        branch.current_agent = Arc::clone(&self.ctx.initial_agent);
        branch.current_world = Arc::clone(&self.ctx.initial_world);
    }

    /// Advances forward verification of a branch by one unit of work, so callbacks and other nodes
    /// can interleave. `Ready` re-queues the node; `Complete` means the chain is verified.
    fn process_simulation(&mut self, node: &mut SearchNode) -> StepResult<()> {
        let branch = &mut node.branch;

        // 0. Initial-state bookkeeping at the start of the chain.
        if branch.simulation_index == 0 {
            self.clear_initial_state_requirements(branch);
        }

        let current_bindings = branch.collect_bindings_for_position(branch.simulation_index);

        // 1. Check open preconditions for current index
        if branch.simulation_index < branch.action_chain.len() {
            let action_idx = branch.action_chain[branch.simulation_index];
            let action = &self.ctx.actions[action_idx];

            if let Some(result) = self.validate_action_against_current_state(
                branch,
                action,
                &current_bindings,
                node.callback_response.as_ref(),
            ) {
                return result;
            }
        }

        if let Some(result) = self.evaluate_open_preconditions_for_position(
            branch,
            &current_bindings,
            node.callback_response.as_ref(),
        ) {
            if matches!(result, StepResult::Ready(())) {
                node.callback_response = None;
            }
            return result;
        }

        // 2. Simulate the current action or finalize the chain.
        if branch.simulation_index < branch.action_chain.len() {
            let action_idx = branch.action_chain[branch.simulation_index];

            let result = self.simulate_and_advance(
                branch,
                action_idx,
                &current_bindings,
                node.callback_response.as_ref(),
            );
            if matches!(result, StepResult::Ready(())) {
                node.callback_response = None;
            }
            return result;
        }

        let result = self.finalize_verified_branch(branch);
        if matches!(result, StepResult::Ready(())) {
            node.callback_response = None;
        }
        result
    }

    /// Removes position-0 requirements already satisfied by the initial state. Runs once at the
    /// start of verification.
    pub fn clear_initial_state_requirements(&self, branch: &mut PlanBranch) {
        branch.open_requirements.retain(|(pos, req)| {
            !(*pos == 0
                && self.ctx.initial_provisions.iter().any(|prov| {
                    provision_satisfies_requirement(prov, req, Some(&self.ctx.initial_world))
                }))
        });
    }

    /// Rejects the branch if an action's built-in preconditions do not hold in the simulated state
    /// before it runs. Custom ones are left to the open-precondition step, which
    /// [`Self::begin_verification`] re-opened them for.
    pub fn validate_action_against_current_state(
        &self,
        branch: &PlanBranch,
        action: &ActionSpec,
        current_bindings: &[(String, Vec<VariantSnapshot>)],
        callback_response: Option<&CallbackResponse>,
    ) -> Option<StepResult<()>> {
        for pre in &action.preconditions {
            if matches!(pre, PreconditionSpec::Custom { .. }) {
                continue;
            }
            match eval_precondition(
                pre,
                &branch.current_agent,
                &branch.current_world,
                &self.ctx,
                callback_response,
                current_bindings,
            ) {
                StepResult::Ready(true) => {}
                StepResult::Ready(false) => {
                    if branch.state == BranchState::Verifying {
                        return Some(StepResult::Invalid);
                    }
                }
                StepResult::Pending(id) => return Some(StepResult::Pending(id)),
                StepResult::Invalid => return Some(StepResult::Invalid),
                StepResult::Complete => unreachable!("eval_precondition cannot return Complete"),
            }
        }
        None
    }

    /// Checks one open precondition at the current position per call (custom ones may need a
    /// callback). Returns `None` when none remain; a false one invalidates the branch.
    pub fn evaluate_open_preconditions_for_position(
        &mut self,
        branch: &mut PlanBranch,
        current_bindings: &[(String, Vec<VariantSnapshot>)],
        callback_response: Option<&CallbackResponse>,
    ) -> Option<StepResult<()>> {
        let mut i = 0;
        while i < branch.open_preconditions.len() {
            if branch.open_preconditions[i].0 == branch.simulation_index {
                match eval_precondition(
                    &branch.open_preconditions[i].1,
                    &branch.current_agent,
                    &branch.current_world,
                    &self.ctx,
                    callback_response,
                    current_bindings,
                ) {
                    StepResult::Ready(true) => {
                        branch.open_preconditions.remove(i);
                        return Some(StepResult::Ready(()));
                    }
                    StepResult::Ready(false) => {
                        if branch.state == BranchState::Verifying {
                            let consumer = match branch.action_chain.get(branch.simulation_index) {
                                Some(&idx) => self.ctx.actions[idx].name.clone(),
                                None => "GOAL".to_string(),
                            };
                            let detail = format!("failed: {}", branch.open_preconditions[i].1);
                            self.tree.add_fwd_step(
                                branch.tree_node_id,
                                &consumer,
                                "precondition",
                                &detail,
                                false,
                            );
                            return Some(StepResult::Invalid);
                        }
                    }
                    StepResult::Pending(id) => return Some(StepResult::Pending(id)),
                    StepResult::Invalid => return Some(StepResult::Invalid),
                    StepResult::Complete => {
                        unreachable!("eval_precondition cannot return Complete")
                    }
                }
            }
            i += 1;
        }
        None
    }

    /// Checks the current action's requirements, simulates it, clears later requirements it
    /// provides, and advances to the next action.
    pub fn simulate_and_advance(
        &mut self,
        branch: &mut PlanBranch,
        action_idx: usize,
        current_bindings: &[(String, Vec<VariantSnapshot>)],
        callback_response: Option<&CallbackResponse>,
    ) -> StepResult<()> {
        if branch.state == BranchState::Verifying {
            // The immutable borrow of the action spec ends inside this block
            // so the tree annotation below can borrow `self` mutably.
            let failed: Option<(String, String)> = {
                let action = &self.ctx.actions[action_idx];
                action
                    .requirements
                    .iter()
                    .find(|req| {
                        !requirement_holds_in_state(
                            req,
                            &branch.current_agent,
                            &branch.current_world,
                            current_bindings,
                        )
                    })
                    .map(|req| (action.name.clone(), format!("failed: {}", req)))
            };
            if let Some((action_name, detail)) = failed {
                self.tree.add_fwd_step(
                    branch.tree_node_id,
                    &action_name,
                    "requirement",
                    &detail,
                    false,
                );
                return StepResult::Invalid;
            }
        }

        let key: StepKey = (
            StateRef(Arc::clone(&branch.current_agent)),
            StateRef(Arc::clone(&branch.current_world)),
            action_idx,
            current_bindings.to_vec(),
        );
        let cost_known = |branch: &PlanBranch| {
            branch
                .action_costs
                .get(branch.simulation_index)
                .is_some_and(|c| c.is_finite() && *c >= 0.0)
        };
        // With the cost known, only the effect is left: reuse another branch's post-state, or
        // wait on its request.
        let cached = if cost_known(branch) {
            if let Some(state) = self.step_cache.get(&key) {
                Some(state.clone())
            } else if callback_response.is_none()
                && let Some(&id) = self.step_pending.get(&key)
            {
                return StepResult::Pending(id);
            } else {
                None
            }
        } else {
            None
        };

        let (agent, world) = match cached {
            Some(state) => state,
            None => match simulate_action(
                action_idx,
                SimArgs {
                    agent: &branch.current_agent,
                    world: &branch.current_world,
                    ctx: &self.ctx,
                    response: callback_response,
                    branch_action_costs: &mut branch.action_costs,
                    simulation_index: branch.simulation_index,
                    bindings: current_bindings,
                },
            ) {
                // No effect: the state is unchanged, so keep sharing it.
                StepResult::Ready(_)
                    if self.ctx.actions[action_idx].effect_callable_id.is_none() =>
                {
                    (
                        Arc::clone(&branch.current_agent),
                        Arc::clone(&branch.current_world),
                    )
                }
                StepResult::Ready(res) => self
                    .step_cache
                    .entry(key)
                    .or_insert_with(|| (Arc::new(res.agent), Arc::new(res.world)))
                    .clone(),
                StepResult::Pending(id) => {
                    // A request sent once the cost is known is the effect: let others share it.
                    if cost_known(branch) && !self.step_pending.contains_key(&key) {
                        self.step_pending.insert(key.clone(), id);
                        self.step_requests.insert(id, key);
                    }
                    return StepResult::Pending(id);
                }
                StepResult::Invalid => return StepResult::Invalid,
                StepResult::Complete => unreachable!("simulate_action cannot return Complete"),
            },
        };
        branch.current_agent = agent;
        branch.current_world = world;

        self.clear_requirements_from_provisions(branch, action_idx, current_bindings);

        branch.simulation_index += 1;
        branch.recalculate_cost();
        StepResult::Ready(())
    }

    /// Clears requirements at or after the current position that this action's provisions satisfy.
    /// Wildcards are first made concrete with this position's binding, so one GoTo only satisfies
    /// `at_target` for its own target.
    pub fn clear_requirements_from_provisions(
        &self,
        branch: &mut PlanBranch,
        action_idx: usize,
        current_bindings: &[(String, Vec<VariantSnapshot>)],
    ) {
        let action = &self.ctx.actions[action_idx];
        let concrete_provisions: Vec<ProvisionSpec> = action
            .provisions
            .iter()
            .map(|prov| concretize_wildcard_provision(prov, current_bindings))
            .collect();

        for prov in &concrete_provisions {
            branch.open_requirements.retain(|(pos, req)| {
                !(*pos >= branch.simulation_index
                    && provision_satisfies_requirement(prov, req, Some(&branch.current_world)))
            });
        }
    }

    /// At the end of the chain: `Complete` if no needs remain (recorded if cheapest), `Invalid`
    /// otherwise. `Ready` outside verification.
    pub fn finalize_verified_branch(&mut self, branch: &mut PlanBranch) -> StepResult<()> {
        match branch.state {
            BranchState::Verifying => {
                if !branch.open_preconditions.is_empty() {
                    self.tree.set_outcome(
                        branch.tree_node_id,
                        NodeOutcome::Pruned {
                            reason: "Unsatisfied preconditions remain".to_string(),
                        },
                    );
                    return StepResult::Invalid;
                }

                if !branch.open_requirements.is_empty() {
                    self.tree.set_outcome(
                        branch.tree_node_id,
                        NodeOutcome::Pruned {
                            reason: "Unsatisfied requirements remain".to_string(),
                        },
                    );
                    return StepResult::Invalid;
                }

                if branch.cost < self.best_cost {
                    self.tree.set_outcome(
                        branch.tree_node_id,
                        NodeOutcome::Complete {
                            chain_len: branch.action_chain.len(),
                            total_cost: branch.cost,
                            fwd_ok: true,
                        },
                    );
                    self.best_cost = branch.cost;
                    self.best_plan = Some(PlanResult {
                        success: true,
                        action_chain: branch.action_chain.iter().map(|&i| i as i64).collect(),
                        total_cost: branch.cost,
                        goal_index: branch.goal_index as i64,
                        action_bindings: branch
                            .action_bindings
                            .iter()
                            .map(|(pos, name, vals)| (*pos as i64, name.clone(), vals.clone()))
                            .collect(),
                    });
                }

                StepResult::Complete
            }
            BranchState::Searching => StepResult::Ready(()),
        }
    }
}

/// Turns a `FactWildcard` into a concrete `Fact` using this position's binding; other
/// provisions are returned unchanged.
fn concretize_wildcard_provision(
    prov: &ProvisionSpec,
    current_bindings: &[(String, Vec<VariantSnapshot>)],
) -> ProvisionSpec {
    if let ProvisionSpec::FactWildcard { fact_name } = prov
        && let Some((_, values)) = current_bindings.iter().find(|(name, _)| name == fact_name)
        && !values.is_empty()
    {
        return ProvisionSpec::Fact {
            fact_name: fact_name.clone(),
            args: values.clone(),
        };
    }
    prov.clone()
}
