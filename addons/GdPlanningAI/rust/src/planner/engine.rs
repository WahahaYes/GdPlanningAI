use crate::plan_types::*;
use crate::plan_tree::PlanResult;
use crate::requirement::{RequirementSpec, ProvisionSpec, provision_satisfies_requirement};
use crate::planner::types::*;
use crate::planner::simulation::{eval_precondition, simulate_action, StepResult};

use crate::planner::expander::find_candidates;
use super::{SearchAlgorithm, TerminationStrategy};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::collections::{HashMap, BinaryHeap, HashSet};
use std::sync::mpsc::{Receiver, Sender};

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
    pub visited: HashMap<(usize, Vec<(usize, PreconditionSpec)>, Vec<(usize, RequirementSpec)>, BranchState), f64>,
    pub best_plan: Option<PlanResult>,
    pub best_cost: f64,
    pub current_goal_index: usize,
}

impl PlannerEngine {
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
        }
    }

    pub fn with_search_algorithm(mut self, alg: SearchAlgorithm) -> Self { 
        self.algorithm = alg;
        self
    }
    pub fn with_termination_strategy(mut self, strat: TerminationStrategy) -> Self { 
        self.termination = strat;
        self
    }

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
        let mut branch = PlanBranch::new(&self.ctx.initial_agent, &self.ctx.initial_world);
        branch.goal_index = goal.original_index; // Use original index for Godot
        for pre in &goal.desired_state {
            branch.open_preconditions.push((0, pre.clone()));
        }
        
        self.queue.push(SearchNode {
            branch,
            resumed: false,
            callback_response: None,
        });
    }

    fn step_search(&mut self, goals: &[GoalSpec]) -> PlannerRunResult {
        // 1. Resume Callbacks
        while let Ok(callback) = self.response_rx.try_recv() {
            // Handle Discovery responses
            let discovery_req = {
                let req_map = self.ctx.discovery_request_map.lock().unwrap();
                req_map.get(&callback.request_id).cloned()
            };

            if let Some(req) = discovery_req {
                match req {
                    DiscoveryRequest::Simulation(idx) => {
                        if let CallbackResponse::UpdatedSnapshots(ref agent, ref world) = callback.response {
                            let mut cache = self.ctx.discovery_results.lock().unwrap();
                            let cost = {
                                let costs = self.ctx.discovery_costs.lock().unwrap();
                                costs.get(&idx).cloned().unwrap_or(1.0)
                            };
                            cache.insert(idx, DiscoveryResult { 
                                agent: agent.clone(), 
                                world: world.clone(), 
                                cost
                            });
                        } else if let CallbackResponse::Float(cost) = callback.response {
                            let mut costs = self.ctx.discovery_costs.lock().unwrap();
                            costs.insert(idx, cost);
                        }

                        let mut pending = self.ctx.discovery_pending.lock().unwrap();
                        pending.remove(&idx);
                    }
                    DiscoveryRequest::Precondition(idx, spec) => {
                        if let CallbackResponse::Bool(b) = callback.response {
                            let mut cache = self.ctx.discovery_precond_results.lock().unwrap();
                            cache.insert((idx, spec.clone()), b);
                        }
                        let mut pending = self.ctx.discovery_precond_pending.lock().unwrap();
                        pending.remove(&(idx, spec));
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
                if self.termination == TerminationStrategy::BestCost && node.priority() >= self.best_cost {
                    self.queue.push(node); // Put it back for next time if needed
                    break;
                }

                // Increase budget for local tests
            if iterations > 5000 {
                self.queue.push(node);
                return PlannerRunResult::Pending(0);
            }

            if self.cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                return PlannerRunResult::Complete(None);
            }

            // Visited check
            if !node.resumed && node.branch.state == BranchState::Searching {
                let fp = node.branch.fingerprint();
                if let Some(&prev_cost) = self.visited.get(&fp) {
                    if node.branch.symbolic_cost >= prev_cost { continue; }
                }
                self.visited.insert(fp, node.branch.symbolic_cost);
            }
            node.resumed = false;

            // 3. State Machine Processing
            if node.branch.symbolic_cost >= self.best_cost { continue; }

            match node.branch.state {
                BranchState::Initializing | BranchState::Rippling | BranchState::Verifying => {
                    match self.process_simulation(&mut node) {
                        StepResult::Ready(_) => self.queue.push(node),
                        StepResult::Pending(id) => {
                            self.parked_nodes.entry(id).or_insert_with(Vec::new).push(node);
                        }
                        StepResult::Invalid => {}
                        StepResult::Complete => {
                            // Verification pass finished and updated best_plan.
                            // If strategy is FirstComplete, we are done.
                            if self.termination == TerminationStrategy::FirstComplete {
                                if let Some(plan) = self.best_plan.take() {
                                    return PlannerRunResult::Complete(Some(plan));
                                }
                            }
                            // Otherwise (BestCost), we just discard this branch and keep searching.
                        }
                    }
                }
                BranchState::Searching => {
                    // Check if Goal Satisfied
                    if node.branch.open_preconditions.is_empty() && node.branch.open_requirements.is_empty() {
                        node.branch.state = BranchState::Verifying;
                        node.branch.simulation_index = 0;
                        node.branch.current_agent = self.ctx.initial_agent.clone();
                        node.branch.current_world = self.ctx.initial_world.clone();
                        // node.branch.cost = 0.0; // DON'T reset cost, keep the symbolic estimate for priority
                        self.queue.push(node);
                        
                        // If we are looking for any plan, we've found our candidate.
                        // But we still need to Verify it.
                        continue;
                    }

                    // Expand
                    if node.branch.action_chain.len() >= self.max_depth { continue; }
                    match find_candidates(&node.branch, &*self.ctx, node.callback_response.as_ref()) {
                        StepResult::Ready(candidates) => {
                            for cand in candidates {
                                let mut new_branch = node.branch.clone();
                                let action = &self.ctx.actions[cand.action_idx];
                                
                                // Prepend action
                                new_branch.action_chain.insert(0, cand.action_idx);
                                let discovery_cost = {
                                    let cache = self.ctx.discovery_results.lock().unwrap();
                                    cache.get(&cand.action_idx).map(|r| r.cost).unwrap_or(1.0)
                                };
                                new_branch.action_costs.insert(0, discovery_cost);
                                new_branch.symbolic_cost += discovery_cost;
                                
                                // Update indices of existing needs and bindings
                                for (pos, _) in new_branch.open_preconditions.iter_mut() { *pos += 1; }
                                for (pos, _) in new_branch.open_requirements.iter_mut() { *pos += 1; }
                                for (pos, _, _) in new_branch.action_bindings.iter_mut() { *pos += 1; }

                                // 1. Record and remove satisfied requirements
                                let mut new_bindings = Vec::new();
                                let mut reqs_to_remove: HashSet<usize> = cand.satisfied_requirements.iter().map(|(idx, _, _)| *idx).collect();
                                for (_, req, prov) in cand.satisfied_requirements {
                                    if let ProvisionSpec::Binding { binding_name, value } = prov {
                                        new_bindings.push((0, binding_name, vec![value]));
                                    } else if let ProvisionSpec::Fact { fact_name, args } = prov {
                                        new_bindings.push((0, fact_name, args));
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
                                let mut preconds_to_remove: HashSet<usize> = cand.satisfied_preconditions.iter().map(|idx| *idx).collect();
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
                                
                                // Add the new bindings for the prepended action
                                new_branch.action_bindings.extend(new_bindings);

                                // Add new needs from the prepended action at pos=0
                                for pre in &action.preconditions {
                                    new_branch.open_preconditions.push((0, pre.clone()));
                                }
                                for req in &action.requirements {
                                    new_branch.open_requirements.push((0, req.clone()));
                                }

                                // Reset to Rippling to check if this action satisfies anything
                                new_branch.state = BranchState::Rippling;
                                new_branch.simulation_index = 0;
                                new_branch.current_agent = self.ctx.initial_agent.clone();
                                new_branch.current_world = self.ctx.initial_world.clone();
                                new_branch.recalculate_cost();
                                
                                self.queue.push(SearchNode {
                                    branch: new_branch,
                                    resumed: false,
                                    callback_response: None,
                                });
                            }
                        }
                        StepResult::Pending(id) => {
                            self.parked_nodes.entry(id).or_insert_with(Vec::new).push(node);
                        }
                        StepResult::Invalid => {}
                        StepResult::Complete => unreachable!("find_candidates cannot return Complete"),
                    }
                }
            }
        }

        if !self.parked_nodes.is_empty() {
            PlannerRunResult::Pending(*self.parked_nodes.keys().next().unwrap())
        } else {
            // Search exhausted for current goal. Check if there are more goals.
            if self.best_plan.is_none() && self.current_goal_index + 1 < goals.len() {
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
            PlannerRunResult::Complete(Some(final_plan))
        }
    }

    #[allow(unused_assignments)]
    fn process_simulation(&mut self, node: &mut SearchNode) -> StepResult<()> {
        let branch = &mut node.branch;
        
        // 1. Check open preconditions for current index
        let mut i = 0;
        while i < branch.open_preconditions.len() {
            if branch.open_preconditions[i].0 == branch.simulation_index {
                match eval_precondition(&branch.open_preconditions[i].1, &branch.current_agent, &branch.current_world, &*self.ctx, node.callback_response.as_ref()) {
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
                    StepResult::Complete => unreachable!("eval_precondition cannot return Complete"),
                }
            }
            i += 1;
        }

        // 2. Step simulation forward
        if branch.simulation_index < branch.action_chain.len() {
            let action_idx = branch.action_chain[branch.simulation_index];
            match simulate_action(action_idx, &branch.current_agent, &branch.current_world, &*self.ctx, node.callback_response.as_ref(), &mut branch.action_costs, branch.simulation_index) {
                StepResult::Ready(res) => {
                    branch.current_agent = res.agent;
                    branch.current_world = res.world;
                    // branch.cost += res.cost; // Handled by action_costs and recalculate_cost
                    
                    // Mark requirements satisfied by this action's provisions
                    let action = &self.ctx.actions[action_idx];
                    for prov in &action.provisions {
                        branch.open_requirements.retain(|(pos, req)| {
                            !(*pos > branch.simulation_index && provision_satisfies_requirement(prov, req, None))
                        });
                    }

                    branch.simulation_index += 1;
                    branch.recalculate_cost();
                    node.callback_response = None;
                    return StepResult::Ready(());
                }
                StepResult::Pending(id) => return StepResult::Pending(id),
                StepResult::Invalid => return StepResult::Invalid,
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
                    branch.state = BranchState::Searching;
                    return StepResult::Ready(());
                }
                BranchState::Verifying => {
                    // Final success!
                    if branch.cost < self.best_cost {
                        self.best_cost = branch.cost;
                        self.best_plan = Some(PlanResult {
                            success: true,
                            action_chain: branch.action_chain.iter().map(|&i| i as i64).collect(),
                            total_cost: branch.cost,
                            goal_index: branch.goal_index as i64,
                            deferred_action_indices: vec![],
                            action_bindings: branch.action_bindings.iter()
                                .map(|(pos, name, vals)| (*pos as i64, name.clone(), vals.clone()))
                                .collect(),
                        });
                    }
                    
                    return StepResult::Complete;
                }
                BranchState::Searching => return StepResult::Ready(()),
            }
        }

        // If we reached here, it means we checked all preconds at this index and none were met,
        // and we either simulated forward or we are stuck.
        // In Verifying, if any preconds at this index are still open, it's a failure.
        if branch.state == BranchState::Verifying {
            let has_open_here = branch.open_preconditions.iter().any(|(pos, _)| *pos == branch.simulation_index);
            if has_open_here {
                return StepResult::Invalid;
            }
        }
        
        StepResult::Ready(())
    }
}
