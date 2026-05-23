use crate::plan_types::*;
use crate::plan_tree::PlanResult;
use super::types::{PlanBranch, CompleteResult};
use super::expander::{SearchContext, BranchExpander, ExpandResult};
use super::controller::{SearchNode, SearchAlgorithm, TerminationStrategy, create_controller, SearchController};
use super::heuristic;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};

pub struct PlannerEngine {
    ctx: Arc<SearchContext>,
    max_depth: usize,
    cancel_flag: Arc<AtomicBool>,
    search_algorithm: SearchAlgorithm,
    termination_strategy: TerminationStrategy,
    
    // Search state for suspend/resume
    controller: Option<Box<dyn SearchController + Send>>,
    parked_nodes: HashMap<usize, SearchNode>,
    pub response_rx: Receiver<PlannerCallback>,
    pub response_tx: Sender<PlannerCallback>,
    current_goal_idx: usize,
    sorted_goals: Vec<GoalSpec>,
    best_cost: f64,
    best_result: Option<PlanResult>,

    // Profiling metrics
    nodes_explored: usize,
    max_depth_reached: usize,
    candidates_evaluated: usize,
    search_iterations: usize,
}

impl PlannerEngine {
    pub fn new(
        ctx: Arc<SearchContext>,
        max_depth: usize,
        cancel_flag: Arc<AtomicBool>,
    ) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Self {
            ctx,
            max_depth,
            cancel_flag,
            search_algorithm: SearchAlgorithm::AStar,
            termination_strategy: TerminationStrategy::FirstComplete,
            controller: None,
            parked_nodes: HashMap::new(),
            response_rx: rx,
            response_tx: tx,
            current_goal_idx: 0,
            sorted_goals: Vec::new(),
            best_cost: f64::INFINITY,
            best_result: None,
            nodes_explored: 0,
            max_depth_reached: 0,
            candidates_evaluated: 0,
            search_iterations: 0,
        }
    }

    pub fn with_search_algorithm(mut self, algorithm: SearchAlgorithm) -> Self {
        self.search_algorithm = algorithm;
        self
    }

    pub fn with_termination_strategy(mut self, strategy: TerminationStrategy) -> Self {
        self.termination_strategy = strategy;
        self
    }

    pub fn plan(&mut self, goals: &[GoalSpec]) -> PlannerRunResult {
        // Initialization if not already started
        if self.sorted_goals.is_empty() {
            let mut sorted = goals.to_vec();
            sorted.sort_by(|a, b| b.reward.partial_cmp(&a.reward).unwrap_or(std::cmp::Ordering::Equal));
            self.sorted_goals = sorted;
            self.current_goal_idx = 0;
            self.best_cost = f64::INFINITY;
            self.best_result = None;
        }

        while self.current_goal_idx < self.sorted_goals.len() {
            if self.cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                return PlannerRunResult::Complete(None);
            }

            let goal = &self.sorted_goals[self.current_goal_idx].clone();
            if goal.reward <= 0.0 {
                self.current_goal_idx += 1;
                continue;
            }

            match self.step_search(goal) {
                PlannerRunResult::Complete(Some(res)) => return PlannerRunResult::Complete(Some(res)),
                PlannerRunResult::Complete(None) => {
                    self.current_goal_idx += 1;
                    self.controller = None; // Reset for next goal
                }
                PlannerRunResult::Pending(id) => return PlannerRunResult::Pending(id),
            }
        }

        PlannerRunResult::Complete(self.best_result.take().or_else(|| {
            Some(PlanResult {
                success: false,
                action_chain: vec![],
                total_cost: 0.0,
                goal_index: -1,
                deferred_action_indices: vec![],
                action_bindings: vec![],
            })
        }))
    }

    fn step_search(&mut self, goal: &GoalSpec) -> PlannerRunResult {
        // 1. Check for unblocked nodes first (Resumption-First Policy)
        while let Ok(callback) = self.response_rx.try_recv() {
            log_debug!("Received callback response for request {}", callback.request_id);
            
            // Store result in context so simulation can see it
            {
                let mut results = self.ctx.callback_results.lock().unwrap();
                results.insert(callback.sim_key, callback.response);
            }

            // Remove from pending so simulation layer knows it's ready
            {
                let mut pending = self.ctx.pending_requests.lock().unwrap();
                pending.remove(&callback.sim_key);
            }

            if let Some(node) = self.parked_nodes.remove(&callback.request_id) {
                log_debug!("Resuming parked node for request {}", callback.request_id);
                self.controller.as_mut().unwrap().push(node);
            }
        }

        let mut controller = if let Some(c) = self.controller.take() {
            c
        } else {
            log_info!("PlannerEngine: Starting search for goal '{}'", goal.name);
            let mut c = create_controller(self.search_algorithm);
            match PlanBranch::new(
                &goal.desired_state,
                &self.ctx.initial_provisions,
                &self.ctx.initial_agent,
                &self.ctx.initial_world,
                &*self.ctx,
            ) {
                Ok(root) => {
                    c.push(SearchNode {
                        branch: root,
                        depth: 0,
                        estimated_remaining: 0.0,
                    });
                }
                Err(id) => {
                    // Root is pending
                    self.controller = Some(c);
                    return PlannerRunResult::Pending(id);
                }
            }
            c
        };

        let expander = BranchExpander { ctx: self.ctx.clone() };

        while let Some(node) = controller.pop() {
            self.search_iterations += 1;
            self.nodes_explored += 1;
            if node.depth > self.max_depth_reached {
                self.max_depth_reached = node.depth;
            }

            if self.cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                self.controller = Some(controller);
                return PlannerRunResult::Complete(None);
            }

            if node.branch.cost >= self.best_cost {
                continue;
            }

            match node.branch.is_complete(&self.ctx.initial_agent, &self.ctx.initial_world, &*self.ctx) {
                CompleteResult::Ready(true) => {
                    let cost = node.branch.cost;
                    log_info!("Found complete plan for goal '{}' with cost {}", goal.name, cost);

                    match self.termination_strategy {
                        TerminationStrategy::FirstComplete => {
                            self.log_profiling_metrics(goal);
                            return PlannerRunResult::Complete(Some(PlanResult {
                                success: true,
                                action_chain: node.branch.action_chain.clone(),
                                total_cost: cost,
                                goal_index: goal.original_index as i64,
                                deferred_action_indices: vec![],
                                action_bindings: node.branch.action_bindings.clone(),
                            }));
                        }
                        TerminationStrategy::BestCost => {
                            if cost < self.best_cost {
                                self.best_cost = cost;
                                self.best_result = Some(PlanResult {
                                    success: true,
                                    action_chain: node.branch.action_chain.clone(),
                                    total_cost: cost,
                                    goal_index: goal.original_index as i64,
                                    deferred_action_indices: vec![],
                                    action_bindings: node.branch.action_bindings.clone(),
                                });
                            }
                        }
                    }
                }
                CompleteResult::Pending(id) => {
                    self.parked_nodes.insert(id, node);
                    self.controller = Some(controller);
                    return PlannerRunResult::Pending(id);
                }
                _ => {}
            }

            if node.depth >= self.max_depth {
                continue;
            }

            match expander.expand(&node.branch) {
                ExpandResult::Ready(successors) => {
                    self.candidates_evaluated += successors.len();
                    for succ in successors {
                        let h = match self.search_algorithm {
                            SearchAlgorithm::DepthFirst => 0.0,
                            SearchAlgorithm::Dijkstra => 0.0,
                            SearchAlgorithm::AStar => heuristic::estimate_remaining(&succ, 1.0),
                        };
                        controller.push(SearchNode {
                            branch: succ,
                            depth: node.depth + 1,
                            estimated_remaining: h,
                        });
                    }
                }
                ExpandResult::Pending(id) => {
                    self.parked_nodes.insert(id, node);
                    self.controller = Some(controller);
                    return PlannerRunResult::Pending(id);
                }
            }
        }

        self.log_profiling_metrics(goal);
        self.controller = Some(controller);
        PlannerRunResult::Complete(None)
    }

    fn log_profiling_metrics(&self, goal: &GoalSpec) {
        log_info!(
            "=== PROFILING METRICS ===\n  Goal: {}\n  Iterations: {}\n  Nodes Explored: {}\n  Max Depth: {}\n========================",
            goal.name,
            self.search_iterations,
            self.nodes_explored,
            self.max_depth_reached
        );
    }
}
