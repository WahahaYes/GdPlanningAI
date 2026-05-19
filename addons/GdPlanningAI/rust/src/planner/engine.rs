use crate::plan_types::*;
use crate::plan_tree::PlanResult;
use super::types::PlanBranch;
use super::expander::{SearchContext, BranchExpander};
use super::controller::{SearchNode, SearchAlgorithm, TerminationStrategy, create_controller};
use super::heuristic;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub struct PlannerEngine<'a> {
    ctx: &'a SearchContext<'a>,
    max_depth: usize,
    cancel_flag: Arc<AtomicBool>,
    search_algorithm: SearchAlgorithm,
    termination_strategy: TerminationStrategy,
    // Profiling metrics
    nodes_explored: usize,
    max_depth_reached: usize,
    candidates_evaluated: usize,
    search_iterations: usize,
}

impl<'a> PlannerEngine<'a> {
    pub fn new(
        ctx: &'a SearchContext<'a>,
        max_depth: usize,
        cancel_flag: Arc<AtomicBool>,
    ) -> Self {
        Self {
            ctx,
            max_depth,
            cancel_flag,
            search_algorithm: SearchAlgorithm::AStar,
            termination_strategy: TerminationStrategy::FirstComplete,
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

    pub fn plan(&mut self, goals: &[GoalSpec]) -> Option<PlanResult> {
        let mut best_failure: Option<PlanResult> = None;

        // 1. Sort goals by reward (descending)
        let mut sorted_goals = goals.to_vec();
        sorted_goals.sort_by(|a, b| b.reward.partial_cmp(&a.reward).unwrap_or(std::cmp::Ordering::Equal));

        for goal in sorted_goals {
            if self.cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                return None;
            }

            // 2. Ignore goals with zero or negative reward
            if goal.reward <= 0.0 {
                continue;
            }

            log_debug!("PlannerEngine: Searching for goal '{}' (reward: {})", goal.name, goal.reward);

            if let Some(result) = self.search_goal(&goal) {
                if result.success {
                    log_info!("PlannerEngine: Found valid plan for goal '{}' (cost: {})", goal.name, result.total_cost);
                    return Some(result);
                } else {
                    log_debug!("PlannerEngine: Failed to find plan for goal '{}'", goal.name);
                    if best_failure.is_none() {
                        best_failure = Some(result);
                    }
                }
            }
        }
        
        // If we found no success but had a failure result, return that.
        // Otherwise, return a generic failure.
        best_failure.or_else(|| {
            Some(PlanResult {
                success: false,
                action_chain: vec![],
                total_cost: 0.0,
                goal_index: -1,
                deferred_action_indices: vec![],
                action_bindings: vec![],
            })
        })
    }

    fn search_goal(&mut self, goal: &GoalSpec) -> Option<PlanResult> {
        log_info!("PlannerEngine: Searching for goal '{}' with algorithm {:?}", goal.name, self.search_algorithm);
        let mut controller = create_controller(self.search_algorithm);
        
        let root = PlanBranch::new(
            &goal.desired_state,
            self.ctx.initial_provisions,
            self.ctx.initial_agent,
            self.ctx.initial_world,
            self.ctx.request_tx,
        );

        log_info!("  Root branch: {} open preconds, {} open requirements", root.open_preconditions.len(), root.open_requirements.len());
        for (i, p) in root.open_preconditions.iter().enumerate() {
            log_info!("    Precond {}: {:?}", i, p);
        }

        if root.open_preconditions.is_empty() && root.open_requirements.is_empty() {
            // Check if goal is satisfied in initial state deep simulation
            let deep_satisfied = goal.desired_state.iter().all(|p| {
                crate::planner::simulation::eval_precondition(
                    p,
                    self.ctx.initial_agent,
                    self.ctx.initial_world,
                    self.ctx.initial_provisions.to_vec(),
                    vec![], // No bindings for initial state check
                    self.ctx.request_tx,
                )
            });
            if deep_satisfied {
                log_debug!("Goal '{}' already satisfied in initial state; returning empty success", goal.name);
                return Some(PlanResult {
                    success: true,
                    action_chain: vec![],
                    total_cost: 0.0,
                    goal_index: goal.original_index as i64,
                    deferred_action_indices: vec![],
                    action_bindings: vec![],
                });
            }
        }

        controller.push(SearchNode {
            branch: root,
            depth: 0,
            estimated_remaining: 0.0,
        });

        let expander = BranchExpander { ctx: self.ctx };
        let mut best_cost = f64::INFINITY;
        let mut best_result: Option<PlanResult> = None;

        log_info!("Starting search loop with max_depth {}", self.max_depth);

        while let Some(node) = controller.pop() {
            self.search_iterations += 1;
            self.nodes_explored += 1;
            if node.depth > self.max_depth_reached {
                self.max_depth_reached = node.depth;
            }

            if self.search_iterations % 100 == 0 {
                log_info!("Search progress: iteration {}, nodes explored {}, max depth reached {}", 
                    self.search_iterations, self.nodes_explored, self.max_depth_reached);
            }

            if self.cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                log_info!("Search cancelled by cancel_flag after {} iterations", self.search_iterations);
                self.log_profiling_metrics(goal);
                return None;
            }

            // Cost pruning: skip nodes that exceed best cost found so far
            // This is critical for DFS performance (matches old planner behavior)
            if node.branch.cost >= best_cost {
                log_debug!("Pruning node with cost {} >= best {}", node.branch.cost, best_cost);
                continue;
            }

            log_info!("Search iteration: pop node with chain length {}", node.branch.action_chain.len());

            if node.branch.is_complete(self.ctx.initial_agent, self.ctx.initial_world, self.ctx.request_tx) {
                let cost = node.branch.cost;
                log_info!("Found complete plan for goal '{}' with cost {}", goal.name, cost);

                match self.termination_strategy {
                    TerminationStrategy::FirstComplete => {
                        self.log_profiling_metrics(goal);
                        return Some(PlanResult {
                            success: true,
                            action_chain: node.branch.action_chain.clone(),
                            total_cost: cost,
                            goal_index: goal.original_index as i64,
                            deferred_action_indices: vec![],
                            action_bindings: node.branch.action_bindings.clone(),
                        });
                    }
                    TerminationStrategy::BestCost => {
                        if cost < best_cost {
                            best_cost = cost;
                            best_result = Some(PlanResult {
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

            if node.depth >= self.max_depth {
                continue;
            }

            let successors = expander.expand(&node.branch);
            self.candidates_evaluated += successors.len();
            log_info!("Node expansion: chain length {}, depth {}, generated {} successors", 
                node.branch.action_chain.len(), node.depth, successors.len());
            
            if successors.is_empty() {
                log_info!("No successors generated - expander returned empty array");
            }
            
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

        log_info!("Search loop exhausted after {} iterations (no more nodes to explore)", self.search_iterations);
        self.log_profiling_metrics(goal);
        best_result
    }

    fn log_profiling_metrics(&self, goal: &GoalSpec) {
        let num_actions = self.ctx.actions.len();
        let theoretical_max = if num_actions > 0 && self.max_depth > 0 {
            // Theoretical max: num_actions^max_depth (worst case, no pruning)
            num_actions.pow(self.max_depth as u32)
        } else {
            0
        };

        log_info!(
            "=== PROFILING METRICS ===\n  Goal: {}\n  Actions: {}\n  Max Depth: {}\n  Theoretical Max Search Space: {}\n  Search Iterations: {}\n  Nodes Explored: {}\n  Max Depth Reached: {}\n  Candidates Evaluated: {}\n  Exploration Percentage: {:.2}%\n========================",
            goal.name,
            num_actions,
            self.max_depth,
            theoretical_max,
            self.search_iterations,
            self.nodes_explored,
            self.max_depth_reached,
            self.candidates_evaluated,
            if theoretical_max > 0 { (self.nodes_explored as f64 / theoretical_max as f64) * 100.0 } else { 0.0 }
        );
    }
}
