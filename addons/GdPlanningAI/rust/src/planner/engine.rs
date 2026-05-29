//! The core planning engine for GdPlanningAI.
//!
//! This engine implements a hybrid backward-chaining GOAP planner that combines
//! symbolic causal links (Requirements/Provisions) with rich scene simulation
//! (simulate_effect, eval_precondition, calculate_cost).
//!
//! The planning process is non-blocking and uses an A* search algorithm to find
//! the optimal sequence of actions to satisfy a goal.

use crate::debug_tree::{NodeOutcome, TreeDump};
use crate::plan_tree::PlanResult;
use crate::plan_types::*;
use crate::planner::simulation::{SimArgs, StepResult, eval_precondition, simulate_action};
use crate::planner::types::*;
use crate::requirement::{ProvisionSpec, RequirementSpec, provision_satisfies_requirement};
use crate::snapshot::VariantSnapshot;

use super::{SearchAlgorithm, TerminationStrategy};
use crate::planner::expander::find_candidates;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, Sender};

/// The execution engine for the GOAP planner.
///
/// This engine manages the A* search queue, handles Godot callbacks, and
/// orchestrates the simulation of action chains.
pub struct PlannerEngine {
    pub ctx: Arc<SearchContext>,
    pub max_depth: usize,
    pub cancel_flag: Arc<AtomicBool>,
    pub response_rx: Receiver<PlannerCallback>,
    pub response_tx: Sender<PlannerCallback>,

    // Config
    pub algorithm: SearchAlgorithm,
    pub termination: TerminationStrategy,

    // Search State
    pub queue: BinaryHeap<SearchNode>,
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
            algorithm: SearchAlgorithm::AStar,
            termination: TerminationStrategy::BestCost,
            queue: BinaryHeap::new(),
            parked_nodes: HashMap::new(),
            visited: HashMap::new(),
            best_plan: None,
            best_cost: f64::INFINITY,
            current_goal_index: 0,
            tree: TreeDump::new(),
        }
    }

    /// Sets the search algorithm to use (e.g., AStar, BFS).
    pub fn with_search_algorithm(mut self, alg: SearchAlgorithm) -> Self {
        self.algorithm = alg;
        self
    }
    /// Sets the termination strategy (e.g., FirstComplete, BestCost).
    pub fn with_termination_strategy(mut self, strat: TerminationStrategy) -> Self {
        self.termination = strat;
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
            branch.open_preconditions.push((0, pre.clone()));
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

        self.queue.push(SearchNode {
            branch,
            resumed: false,
            callback_response: None,
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
                    self.queue.push(node);
                }
            }
        }

        // 2. Main Search Loop
        let mut iterations = 0;
        while let Some(mut node) = self.queue.pop() {
            iterations += 1;

            // Optimality check: If the best node's priority (g + h) is already worse than our best plan,
            // and we want the best cost, we can stop.
            if self.termination == TerminationStrategy::BestCost
                && node.priority() >= self.best_cost
            {
                self.queue.push(node); // Put it back for next time if needed
                break;
            }

            // Increase budget for local tests
            if iterations > 20000 {
                log_warn!("Search budget exceeded (20000 iterations). Search is taking too long.");
                self.queue.push(node);
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
            node.resumed = false;

            // 3. State Machine Processing
            if node.branch.cost >= self.best_cost {
                continue;
            }

            match node.branch.state {
                BranchState::Initializing | BranchState::Rippling | BranchState::Verifying => {
                    match self.process_simulation(&mut node) {
                        StepResult::Ready(_) => self.queue.push(node),
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
                                && let Some(plan) = self.best_plan.take()
                            {
                                return PlannerRunResult::Complete(Some(plan));
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
                        self.queue.push(node);
                        continue;
                    }

                    // Expand
                    if node.branch.action_chain.len() >= self.max_depth {
                        continue;
                    }
                    let candidates_res = find_candidates(&node.branch, &self.ctx, node.callback_response.as_ref());
                    
                    for cand in candidates_res.ready {
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

                        // Prepend action
                        new_branch.action_chain.insert(0, cand.action_idx);
                        let discovery_cost = {
                            let cache = self.ctx.discovery_results.lock().unwrap();
                            cache
                                .get(&(cand.action_idx, cand.bindings.clone()))
                                .map(|r| r.cost)
                                .unwrap_or(1.0)
                        };
                        new_branch.action_costs.insert(0, discovery_cost);

                        // Update indices of existing needs and bindings
                        for (pos, _) in new_branch.open_preconditions.iter_mut() {
                            *pos += 1;
                        }
                        for (pos, _) in new_branch.open_requirements.iter_mut() {
                            *pos += 1;
                        }
                        for (pos, _, _) in new_branch.action_bindings.iter_mut() {
                            *pos += 1;
                        }

                        // 1. Record and remove satisfied requirements
                        let mut new_bindings = Vec::new();
                        let mut reqs_to_remove = HashSet::new();

                        for (req_idx_in_branch, req, prov) in cand.satisfied_requirements {
                            // GREEDY CLEARING: Find ALL identical requirements in the chain
                            // This prevents multiple actions from piling up the same at_target(...) need.
                            for (idx, (_, other_req)) in new_branch.open_requirements.iter().enumerate() {
                                if other_req == &req {
                                    reqs_to_remove.insert(idx);
                                }
                            }

                            // Determine the binding values based on the provision and the specific requirement it filled
                            let (binding_name, values) = match (&prov, &req) {
                                (
                                    ProvisionSpec::Binding {
                                        binding_name,
                                        value,
                                    },
                                    _,
                                ) => (binding_name.clone(), vec![value.clone()]),
                                (ProvisionSpec::Fact { fact_name, args }, _) => {
                                    (fact_name.clone(), args.clone())
                                }
                                (
                                    ProvisionSpec::FactWildcard { fact_name },
                                    RequirementSpec::Fact { args, .. },
                                ) => (fact_name.clone(), args.clone()),
                                _ => (String::new(), vec![]),
                            };

                            if !binding_name.is_empty() {
                                // Associate with provider (the newly prepended action at pos 0)
                                new_bindings.push((0, binding_name.clone(), values.clone()));
                                
                                // Associate with ALL cleared consumers (their positions were already offset by 1)
                                for &idx in &reqs_to_remove {
                                    let consumer_pos = new_branch.open_requirements[idx].0;
                                    new_bindings.push((consumer_pos, binding_name.clone(), values.clone()));
                                }
                            }
                        }

                        let mut j = 0;
                        let mut removed_count = 0;
                        while j < new_branch.open_requirements.len() {
                            if reqs_to_remove.contains(&(j + removed_count)) {
                                new_branch.open_requirements.remove(j);
                                removed_count += 1;
                            } else {
                                j += 1;
                            }
                        }

                        // 2. Remove satisfied preconditions
                        let preconds_to_remove: HashSet<usize> =
                            cand.satisfied_preconditions.iter().copied().collect();
                        let mut k = 0;
                        let mut removed_pre_count = 0;
                        while k < new_branch.open_preconditions.len() {
                            if preconds_to_remove.contains(&(k + removed_pre_count)) {
                                new_branch.open_preconditions.remove(k);
                                removed_pre_count += 1;
                            } else {
                                k += 1;
                            }
                        }

                        // 3. Add any new needs from the prepended action
                        // Deduplicate against existing needs to prevent congestion
                        let existing_pre: HashSet<PreconditionSpec> = new_branch.open_preconditions.iter().map(|(_, p)| p.clone()).collect();
                        for pre in &action.preconditions {
                            if !existing_pre.contains(pre) {
                                new_branch.open_preconditions.push((0, pre.clone()));
                            }
                        }

                        let existing_req: HashSet<RequirementSpec> = new_branch.open_requirements.iter().map(|(_, r)| r.clone()).collect();
                        for req in &action.requirements {
                            if !existing_req.contains(req) {
                                new_branch.open_requirements.push((0, req.clone()));
                            }
                        }

                        // 4. Merge action bindings
                        new_branch.action_bindings.extend(new_bindings);

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

                        // Check if this action satisfied its own preconditions or goal preconditions
                        // using its discovery simulation result.
                        if let Some(disc_res) = {
                            let cache = self.ctx.discovery_results.lock().unwrap();
                            cache.get(&(cand.action_idx, cand.bindings.clone())).cloned()
                        } {
                            // Clear any preconditions at pos 0 that are satisfied by the state BEFORE the chain
                            // or by the newly prepended action.
                            let mut i = 0;
                            while i < new_branch.open_preconditions.len() {
                                let (pos, pre) = &new_branch.open_preconditions[i];
                                if *pos == 0 {
                                    if let Some(true) = pre.evaluate_builtin(&disc_res.agent, &disc_res.world) {
                                        new_branch.open_preconditions.remove(i);
                                        continue;
                                    }
                                }
                                i += 1;
                            }
                        }

                        // If ALL needs are satisfied, move to Verifying
                        if new_branch.open_preconditions.is_empty() && new_branch.open_requirements.is_empty() {
                            new_branch.state = BranchState::Verifying;
                            new_branch.simulation_index = 0;
                            new_branch.current_agent = self.ctx.initial_agent.clone();
                            new_branch.current_world = self.ctx.initial_world.clone();
                        }

                        self.queue.push(SearchNode {
                            branch: new_branch,
                            resumed: false,
                            callback_response: None,
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

    #[allow(unused_assignments)]
    fn process_simulation(&mut self, node: &mut SearchNode) -> StepResult<()> {
        let branch = &mut node.branch;

        // 0. Check open requirements for current index against InitialState or previous action
        if branch.simulation_index == 0 {
            // InitialState provisions satisfy requirements at pos 0
            branch.open_requirements.retain(|(pos, req)| {
                !(*pos == 0
                    && self.ctx.initial_provisions.iter().any(|prov| {
                        provision_satisfies_requirement(prov, req, Some(&self.ctx.initial_world))
                    }))
            });
        }

        // Gather bindings for current simulation_index
        let current_bindings: Vec<(String, Vec<VariantSnapshot>)> = branch
            .action_bindings
            .iter()
            .filter(|(pos, _, _)| *pos == branch.simulation_index)
            .map(|(_, name, vals)| (name.clone(), vals.clone()))
            .collect();

        // 1. Check open preconditions for current index
        let mut i = 0;
        while i < branch.open_preconditions.len() {
            if branch.open_preconditions[i].0 == branch.simulation_index {
                match eval_precondition(
                    &branch.open_preconditions[i].1,
                    &branch.current_agent,
                    &branch.current_world,
                    &self.ctx,
                    node.callback_response.as_ref(),
                    &current_bindings,
                ) {
                    StepResult::Ready(true) => {
                        branch.open_preconditions.remove(i);
                        node.callback_response = None;
                        // We satisfied one precond. Instead of looping, we return Ready
                        // so the engine re-queues us and we check the next one in the next iteration.
                        // This is slightly slower but MUCH safer for async.
                        return StepResult::Ready(());
                    }
                    StepResult::Ready(false) => {
                        // Not satisfied yet, keep it and check others at this index.
                    }
                    StepResult::Pending(id) => return StepResult::Pending(id),
                    StepResult::Invalid => return StepResult::Invalid,
                    StepResult::Complete => {
                        unreachable!("eval_precondition cannot return Complete")
                    }
                }
            }
            i += 1;
        }

        // 2. Step simulation forward
        if branch.simulation_index < branch.action_chain.len() {
            let action_idx = branch.action_chain[branch.simulation_index];
            match simulate_action(
                action_idx,
                SimArgs {
                    agent: &branch.current_agent,
                    world: &branch.current_world,
                    ctx: &self.ctx,
                    response: node.callback_response.as_ref(),
                    branch_action_costs: &mut branch.action_costs,
                    simulation_index: branch.simulation_index,
                    bindings: &current_bindings,
                },
            ) {
                StepResult::Ready(res) => {
                    branch.current_agent = res.agent;
                    branch.current_world = res.world;
                    // branch.cost += res.cost; // Handled by action_costs and recalculate_cost

                    // Mark requirements satisfied by this action's provisions
                    let action = &self.ctx.actions[action_idx];
                    for prov in &action.provisions {
                        branch.open_requirements.retain(|(pos, req)| {
                            !(*pos > branch.simulation_index
                                && provision_satisfies_requirement(prov, req, None))
                        });
                    }

                    branch.simulation_index += 1;
                    branch.recalculate_cost();
                    node.callback_response = None;
                    StepResult::Ready(())
                }
                StepResult::Pending(id) => StepResult::Pending(id),
                StepResult::Invalid => StepResult::Invalid,
                StepResult::Complete => unreachable!("simulate_action cannot return Complete"),
            }
        } else {
            // Reached end of chain
            match branch.state {
                BranchState::Initializing => {
                    branch.state = BranchState::Searching;
                    return StepResult::Ready(());
                }
                BranchState::Rippling => {
                    // Check if the simulation we just finished satisfied any goal preconditions
                    // (preconditions at the end of the chain)
                    let mut i = 0;
                    while i < branch.open_preconditions.len() {
                        if branch.open_preconditions[i].0 == branch.action_chain.len() {
                            let pre = &branch.open_preconditions[i].1;
                            if let Some(true) =
                                pre.evaluate_builtin(&branch.current_agent, &branch.current_world)
                            {
                                branch.open_preconditions.remove(i);
                                continue;
                            }
                        }
                        i += 1;
                    }

                    branch.state = BranchState::Searching;
                    return StepResult::Ready(());
                }
                BranchState::Verifying => {
                    // Final success!
                    // Check if all goal preconditions were actually met
                    let has_open_preconds = branch
                        .open_preconditions
                        .iter()
                        .any(|(pos, _)| *pos == branch.simulation_index);
                    if has_open_preconds {
                        self.tree.set_outcome(
                            branch.tree_node_id,
                            NodeOutcome::Pruned {
                                reason: "Goal preconditions not met".to_string(),
                            },
                        );
                        return StepResult::Invalid;
                    }

                    // Ensure no requirements remain open anywhere in the chain
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

                    return StepResult::Complete;
                }
                BranchState::Searching => {}
            }

            StepResult::Ready(())
        }
    }
}
