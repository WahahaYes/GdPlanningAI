use crate::plan_types::*;
use crate::plan_tree::PlanResult;
use super::types::PlanBranch;
use super::expander::{SearchContext, BranchExpander};
use super::controller::{AStarController, SearchNode};
use super::heuristic;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub struct PlannerEngine<'a> {
    ctx: &'a SearchContext<'a>,
    max_depth: usize,
    cancel_flag: Arc<AtomicBool>,
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
        }
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
        log_info!("PlannerEngine: Searching for goal '{}'", goal.name);
        let mut controller = AStarController::new();
        
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

        while let Some(node) = controller.pop() {
            if self.cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                return None;
            }

            log_info!("A* Iteration: pop node with chain length {}", node.branch.action_chain.len());

            if node.branch.is_complete(self.ctx.initial_agent, self.ctx.initial_world, self.ctx.request_tx) {
                log_info!("A* SUCCESS! Returning plan for goal '{}'", goal.name);
                return Some(PlanResult {
                    success: true,
                    action_chain: node.branch.action_chain,
                    total_cost: node.branch.cost,
                    goal_index: goal.original_index as i64,
                    deferred_action_indices: vec![],
                    action_bindings: node.branch.action_bindings,
                });
            }

            if node.depth >= self.max_depth {
                continue;
            }

            let successors = expander.expand(&node.branch);
            for succ in successors {
                let h = heuristic::estimate_remaining(&succ, 1.0);
                controller.push(SearchNode {
                    branch: succ,
                    depth: node.depth + 1,
                    estimated_remaining: h,
                });
            }
        }

        None
    }
}
