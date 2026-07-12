//! The core planning engine for GdPlanningAI.
//!
//! This engine implements a hybrid backward-chaining GOAP planner that combines
//! symbolic causal links (Requirements/Provisions) with rich scene simulation
//! (simulate_effect, eval_precondition, calculate_cost).
//!
//! The planning process is non-blocking and uses a backward-chaining Dijkstra
//! search to find the optimal sequence of actions to satisfy a goal.

use crate::debug_tree::{NodeOutcome, TreeDump};
use crate::plan_tree::PlanResult;
use crate::plan_types::*;
use crate::planner::simulation::{SimArgs, StepResult, eval_precondition, simulate_action};
use crate::planner::types::*;
use crate::requirement::{
    ProvisionSpec, RequirementSpec, provision_satisfies_requirement, requirement_holds_in_state,
};
use crate::snapshot::VariantSnapshot;

use super::{SearchHeuristic, TerminationStrategy};
use crate::planner::expander::find_candidates;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, Sender};

/// The execution engine for the GOAP planner.
///
/// This engine manages the backward-chaining search queue, handles Godot callbacks, and
/// orchestrates the simulation of action chains.
pub struct PlannerEngine {
    pub ctx: Arc<SearchContext>,
    pub max_depth: usize,
    pub cancel_flag: Arc<AtomicBool>,
    pub response_rx: Receiver<PlannerCallback>,
    pub response_tx: Sender<PlannerCallback>,

    // Config
    pub heuristic: Box<dyn SearchHeuristic + Send + Sync>,
    pub termination: TerminationStrategy,
    pub iteration_budget: usize,

    // Search State
    pub queue: BinaryHeap<PriorityNode>,
    pub parked_nodes: HashMap<usize, Vec<SearchNode>>,
    pub visited: HashMap<SearchFingerprint, f64>,
    pub best_plan: Option<PlanResult>,
    pub best_cost: f64,
    pub current_goal_index: usize,
    pub tree: TreeDump,
}

impl PlannerEngine {
    /// Creates a new PlannerEngine with the given search context and constraints.
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
            queue: BinaryHeap::new(),
            parked_nodes: HashMap::new(),
            visited: HashMap::new(),
            best_plan: None,
            best_cost: f64::INFINITY,
            current_goal_index: 0,
            tree: TreeDump::new(),
        }
    }

    /// Sets the search heuristic to use (e.g., Dijkstra, A*).
    pub fn with_heuristic(mut self, heuristic: Box<dyn SearchHeuristic + Send + Sync>) -> Self {
        self.heuristic = heuristic;
        self
    }

    /// Helper to push a [`SearchNode`] onto the queue with its priority
    /// computed by the active heuristic.
    fn enqueue(&mut self, node: SearchNode) {
        let priority = self.heuristic.compute_priority(&node);
        self.queue.push(PriorityNode { priority, node });
    }
    /// Sets the termination strategy (e.g., FirstComplete, BestCost).
    pub fn with_termination_strategy(mut self, strat: TerminationStrategy) -> Self {
        self.termination = strat;
        self
    }
    /// Sets the iteration budget for a single planning step.
    /// The search yields after this many iterations to avoid blocking the main thread.
    pub fn with_iteration_budget(mut self, budget: usize) -> Self {
        self.iteration_budget = budget;
        self
    }

    /// Executes the planning process for a set of goals.
    ///
    /// This function performs a non-blocking step of the search and returns
    /// a PlannerRunResult indicating if the plan is complete, pending, or failed.
    pub fn plan(&mut self, goals: &[GoalSpec]) -> PlannerRunResult {
        if self.queue.is_empty() && self.parked_nodes.is_empty() && self.best_plan.is_none() {
            // Initializing with the first goal
            if self.current_goal_index < goals.len() {
                self.initialize_goal(goals, self.current_goal_index);
            }
        }

        self.step_search(goals)
    }

    fn initialize_goal(&mut self, goals: &[GoalSpec], idx: usize) {
        let goal = &goals[idx];

        // Tree: Start goal
        let goal_pre_strings: Vec<String> =
            goal.desired_state.iter().map(|p| p.to_string()).collect();
        self.tree
            .begin_goal(&goal.name, goal.reward, &goal_pre_strings);

        let mut branch = PlanBranch::new(&self.ctx.initial_agent, &self.ctx.initial_world);
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

            // Handle Discovery responses
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

            // Handle Parked nodes
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
        while let Some(priority_node) = self.queue.pop() {
            let node_priority = priority_node.priority;
            let mut node = priority_node.node;
            iterations += 1;

            // Optimality check: If the best node's priority (g + h) is already worse than our best plan,
            // and we want the best cost, we can stop.
            if self.termination == TerminationStrategy::BestCost
                && self
                    .heuristic
                    .prune_threshold_met(node_priority, self.best_cost)
            {
                self.enqueue(node); // Put it back for next time if needed
                break;
            }

            // Increase budget for local tests
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

            // Visited check
            if !node.resumed && node.branch.state == BranchState::Searching {
                let fp = node.branch.fingerprint();
                if let Some(&prev_cost) = self.visited.get(&fp)
                    && node.branch.cost >= prev_cost
                {
                    continue;
                }
                self.visited.insert(fp, node.branch.cost);
            }
            if node.resumed {
                node.expanded_candidates.clear();
            }
            node.resumed = false;

            // 3. State Machine Processing
            if node.branch.cost >= self.best_cost {
                continue;
            }

            match node.branch.state {
                BranchState::Verifying => {
                    let sim_idx = node.branch.simulation_index;
                    let action_name = if sim_idx < node.branch.action_chain.len() {
                        self.ctx.actions[node.branch.action_chain[sim_idx]]
                            .name
                            .clone()
                    } else {
                        "<terminal>".to_string()
                    };
                    let proc_res = self.process_simulation(&mut node);
                    log_debug!(
                        "process_simulation result for sim_idx={} action={}: {:?}",
                        sim_idx,
                        action_name,
                        std::mem::discriminant(&proc_res)
                    );
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
                                    return PlannerRunResult::Complete(Some(plan));
                                }
                            }
                            // Otherwise (BestCost), we just discard this branch and keep searching.
                        }
                    }
                }
                BranchState::Searching => {
                    // Check if Goal Satisfied
                    if node.branch.open_preconditions.is_empty()
                        && node.branch.open_requirements.is_empty()
                    {
                        node.branch.state = BranchState::Verifying;
                        node.branch.simulation_index = 0;
                        node.branch.current_agent = self.ctx.initial_agent.clone();
                        node.branch.current_world = self.ctx.initial_world.clone();
                        self.enqueue(node);
                        continue;
                    }

                    // Expand
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
                            &action.name,
                            discovery_cost,
                            node.branch.cost + discovery_cost,
                            &open_pre,
                            &open_req,
                            &satisfied_pre,
                            &satisfied_req,
                        );

                        // Reset search state for the new branch
                        new_branch.state = BranchState::Searching;
                        new_branch.recalculate_cost();

                        // If ALL needs are satisfied, move to Verifying
                        if new_branch.open_preconditions.is_empty()
                            && new_branch.open_requirements.is_empty()
                        {
                            new_branch.state = BranchState::Verifying;
                            new_branch.simulation_index = 0;
                            new_branch.current_agent = self.ctx.initial_agent.clone();
                            new_branch.current_world = self.ctx.initial_world.clone();
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

                    if let Some(id) = candidates_res.pending_id {
                        node.callback_response = None;
                        self.parked_nodes.entry(id).or_default().push(node);
                    }
                }
            }
        }

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

            // Search exhausted or optimal plan found
            let final_plan = self.best_plan.take().unwrap_or_else(|| PlanResult {
                success: false,
                action_chain: vec![],
                total_cost: 0.0,
                goal_index: -1,
                deferred_action_indices: vec![],
                action_bindings: vec![],
            });

            if final_plan.success {
                let action_names: Vec<String> = final_plan
                    .action_chain
                    .iter()
                    .map(|&idx| self.ctx.actions[idx as usize].name.clone())
                    .collect();
                self.tree
                    .end_goal(true, &action_names, final_plan.total_cost);
                if self.tree.is_enabled() {
                    log_debug!("{}", self.tree.format());
                }
            } else {
                self.tree.end_goal(false, &[], 0.0);
                if self.tree.is_enabled() {
                    log_debug!("{}", self.tree.format());
                }
            }

            PlannerRunResult::Complete(Some(final_plan))
        }
    }

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
            let action = &self.ctx.actions[action_idx];

            let result = self.simulate_and_advance(
                branch,
                action_idx,
                action,
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

    /// Removes position-0 open requirements that are already satisfied by the
    /// initial state provisions.
    pub fn clear_initial_state_requirements(&self, branch: &mut PlanBranch) {
        branch.open_requirements.retain(|(pos, req)| {
            !(*pos == 0
                && self.ctx.initial_provisions.iter().any(|prov| {
                    provision_satisfies_requirement(prov, req, Some(&self.ctx.initial_world))
                }))
        });
    }

    /// Re-evaluates an action's built-in preconditions against the current
    /// simulated state. Returns `None` if no short-circuit is required.
    pub fn validate_action_against_current_state(
        &self,
        branch: &PlanBranch,
        action: &ActionSpec,
        current_bindings: &[(String, Vec<VariantSnapshot>)],
        callback_response: Option<&CallbackResponse>,
    ) -> Option<StepResult<()>> {
        let open_pre_for_current: HashSet<PreconditionSpec> = branch
            .open_preconditions
            .iter()
            .filter(|(pos, _)| *pos == branch.simulation_index)
            .map(|(_, pre)| pre.clone())
            .collect();

        for pre in &action.preconditions {
            if open_pre_for_current.contains(pre) || matches!(pre, PreconditionSpec::Custom { .. })
            {
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

    /// Evaluates open preconditions at the current chain position, removing the
    /// first satisfied one and returning `Ready` to keep async processing safe.
    pub fn evaluate_open_preconditions_for_position(
        &self,
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

    /// Simulates the current action, validates its requirements, clears later
    /// requirements satisfied by its provisions, and advances the simulation index.
    pub fn simulate_and_advance(
        &self,
        branch: &mut PlanBranch,
        action_idx: usize,
        action: &ActionSpec,
        current_bindings: &[(String, Vec<VariantSnapshot>)],
        callback_response: Option<&CallbackResponse>,
    ) -> StepResult<()> {
        log_debug!(
            "process_simulation sim_idx={} action={} open_pre={:?} open_req={:?}",
            branch.simulation_index,
            action.name,
            branch
                .open_preconditions
                .iter()
                .map(|(p, s)| format!("{}:{}", p, s))
                .collect::<Vec<_>>(),
            branch
                .open_requirements
                .iter()
                .map(|(p, r)| format!("{}:{}", p, r))
                .collect::<Vec<_>>()
        );

        if branch.state == BranchState::Verifying {
            for req in &action.requirements {
                if !requirement_holds_in_state(
                    req,
                    &branch.current_agent,
                    &branch.current_world,
                    current_bindings,
                ) {
                    log_debug!(
                        "process_simulation requirement failed sim_idx={} action={} req={} bindings={:?}",
                        branch.simulation_index,
                        action.name,
                        req,
                        current_bindings
                    );
                    return StepResult::Invalid;
                }
            }
        }

        match simulate_action(
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
            StepResult::Ready(res) => {
                branch.current_agent = res.agent;
                branch.current_world = res.world;

                self.clear_requirements_from_provisions(branch, action_idx, current_bindings);

                branch.simulation_index += 1;
                branch.recalculate_cost();
                StepResult::Ready(())
            }
            StepResult::Pending(id) => StepResult::Pending(id),
            StepResult::Invalid => StepResult::Invalid,
            StepResult::Complete => unreachable!("simulate_action cannot return Complete"),
        }
    }

    /// Clears later open requirements that are satisfied by the current action's
    /// provisions, concretizing wildcard facts with the chain-position binding.
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

    /// Handles the end of the action chain, pruning remaining open needs or
    /// recording the branch as the best plan found so far.
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
                        deferred_action_indices: vec![],
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

/// Concretizes a [`ProvisionSpec::FactWildcard`] using the current chain-position binding.
///
/// If the provision is not a wildcard, or no binding exists for the wildcard fact name,
/// the original provision is returned unchanged.
fn concretize_wildcard_provision(
    prov: &ProvisionSpec,
    current_bindings: &[(String, Vec<VariantSnapshot>)],
) -> ProvisionSpec {
    if let ProvisionSpec::FactWildcard { fact_name } = prov
        && let Some((_, values)) = current_bindings.iter().find(|(name, _)| name == fact_name)
            && !values.is_empty() {
                return ProvisionSpec::Fact {
                    fact_name: fact_name.clone(),
                    args: values.clone(),
                };
            }
    prov.clone()
}
