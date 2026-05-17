use crate::debug_tree::NodeOutcome;
use crate::plan_tree::PlanResult;
use crate::plan_types::*;
use std::sync::{Arc, atomic::AtomicBool};

use super::controller::{SearchController, SearchNode};
use super::expander::{BranchExpander, PlanBranch, SearchContext};
use super::goal_selection::GoalSelection;
use super::policy::TerminationPolicy;
use super::stats::SearchStats;

pub struct PlannerEngine<'ctx> {
    ctx: &'ctx SearchContext<'ctx>,
    controller: Box<dyn SearchController + 'ctx>,
    policy: Box<dyn TerminationPolicy + 'ctx>,
    goal_selection: Box<dyn GoalSelection + 'ctx>,
    pub stats: SearchStats,
}

impl<'ctx> PlannerEngine<'ctx> {
    pub fn new(
        ctx: &'ctx SearchContext<'ctx>,
        controller: Box<dyn SearchController + 'ctx>,
        policy: Box<dyn TerminationPolicy + 'ctx>,
        goal_selection: Box<dyn GoalSelection + 'ctx>,
    ) -> Self {
        Self {
            ctx,
            controller,
            policy,
            goal_selection,
            stats: SearchStats::new(),
        }
    }

    pub fn run(&mut self) -> Option<PlanResult> {
        let mut best_result: Option<PlanResult> = None;
        let mut best_reward: f64 = 0.0;
        let mut best_cost: f64 = f64::INFINITY;

        let candidates = self.goal_selection.select_goals(self.ctx.goals);

        for candidate in &candidates {
            if self
                .ctx
                .cancel_flag
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                return best_result;
            }

            let goal = &self.ctx.goals[candidate.goal_index];

            let goal_precond_names: Vec<String> = goal
                .desired_state
                .iter()
                .map(|p| format!("{:?}", p))
                .collect();

            self.ctx.tree_dump.borrow_mut().begin_goal(
                &goal.name,
                goal.reward,
                &goal_precond_names,
            );

            let goal_satisfied = goal.desired_state.iter().all(|p| {
                p.evaluate_builtin(self.ctx.initial_agent, self.ctx.initial_world)
                    .unwrap_or_else(|| {
                        super::eval_precondition(
                            p,
                            self.ctx.initial_agent,
                            self.ctx.initial_world,
                            self.ctx.request_tx,
                        )
                    })
            });

            if goal_satisfied && candidate.skip_if_satisfied {
                self.ctx.tree_dump.borrow_mut().goal_already_satisfied();
                self.ctx.tree_dump.borrow_mut().end_goal(true, &[], 0.0);
                let result = PlanResult {
                    success: true,
                    action_chain: vec![],
                    total_cost: 0.0,
                    goal_index: goal.original_index as i64,
                    deferred_action_indices: vec![],
                    action_bindings: vec![],
                };
                if self.goal_selection.short_circuit_on_first_valid() {
                    return Some(result);
                }
                if best_result.is_none()
                    || self
                        .goal_selection
                        .is_better_than(goal.reward, 0.0, best_reward, best_cost)
                {
                    best_reward = goal.reward;
                    best_cost = 0.0;
                    best_result = Some(result);
                    self.stats.best_cost = 0.0;
                }
                continue;
            }

            let root_branch = PlanBranch::new(
                &goal.desired_state,
                self.ctx.initial_provisions,
                self.ctx.initial_agent,
                self.ctx.initial_world,
            );

            if let Some(result) =
                self.search_goal(root_branch, goal.original_index, &goal.desired_state)
            {
                let plan_action_names: Vec<String> = result
                    .action_chain
                    .iter()
                    .map(|&idx| self.ctx.actions[idx as usize].name.clone())
                    .collect();
                self.ctx.tree_dump.borrow_mut().end_goal(
                    true,
                    &plan_action_names,
                    result.total_cost,
                );
                if self.goal_selection.short_circuit_on_first_valid() {
                    return Some(result);
                }
                if best_result.is_none()
                    || self.goal_selection.is_better_than(
                        goal.reward,
                        result.total_cost,
                        best_reward,
                        best_cost,
                    )
                {
                    best_reward = goal.reward;
                    best_cost = result.total_cost;
                    best_result = Some(result);
                    self.stats.best_cost = best_cost;
                }
            } else {
                self.ctx.tree_dump.borrow_mut().end_goal(false, &[], 0.0);
            }
        }

        best_result
    }

    fn search_goal(
        &mut self,
        goal_branch: PlanBranch,
        goal_index: usize,
        goal_preconditions: &[PreconditionSpec],
    ) -> Option<PlanResult> {
        self.policy.on_search_start(self.ctx.max_depth);

        let open_precond_names: Vec<String> = goal_branch
            .open_preconditions
            .iter()
            .map(|p| format!("{:?}", p))
            .collect();
        let root_id = self
            .ctx
            .tree_dump
            .borrow_mut()
            .add_root(&open_precond_names, &[]);

        self.controller.push_initial(SearchNode {
            branch: goal_branch,
            depth: 0,
            estimated_remaining: 0.0,
            tree_node_id: root_id,
        });

        let mut best_result: Option<PlanResult> = None;
        let mut best_cost: f64 = f64::INFINITY;

        loop {
            if self
                .policy
                .should_terminate(&self.stats, self.controller.as_ref())
            {
                break;
            }
            let Some(node) = self.controller.pop_next() else {
                break;
            };
            if self
                .ctx
                .cancel_flag
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                return best_result;
            }

            self.stats.branches_expanded += 1;
            self.stats.max_depth_reached = self.stats.max_depth_reached.max(node.depth);

            crate::log_debug!(
                "Depth {}: {} open preconditions, {} open requirements, est_cost={:.2}, est_rem={:.2}",
                node.depth,
                node.branch.open_preconditions.len(),
                node.branch.open_requirements.len(),
                node.branch.estimated_cost,
                node.estimated_remaining
            );

            if node.depth > self.ctx.max_depth {
                crate::log_debug!("Max depth {} reached", self.ctx.max_depth);
                self.stats.branches_pruned += 1;
                self.ctx.tree_dump.borrow_mut().set_outcome(
                    node.tree_node_id,
                    NodeOutcome::Pruned {
                        reason: "max depth".to_string(),
                    },
                );
                continue;
            }

            let f_score = node.branch.estimated_cost + node.estimated_remaining;
            if f_score >= best_cost {
                crate::log_debug!(
                    "Pruning branch at depth {} with f_score {:.2} (best valid: {:.2})",
                    node.depth,
                    f_score,
                    best_cost
                );
                self.stats.branches_pruned += 1;
                self.ctx.tree_dump.borrow_mut().set_outcome(
                    node.tree_node_id,
                    NodeOutcome::Pruned {
                        reason: format!("f_score {:.2} >= best {:.2}", f_score, best_cost),
                    },
                );
                continue;
            }

            if node.branch.is_complete(self.ctx.request_tx) {
                crate::log_debug!(
                    "Branch complete with {} actions, forward validating...",
                    node.branch.action_chain.len()
                );
                let fwd_result = super::forward_validate(
                    &node.branch.action_chain,
                    &node.branch.action_bindings,
                    self.ctx,
                    goal_preconditions,
                );
                let fwd_ok = fwd_result.is_some();
                if let Some((action_chain, total_cost)) = fwd_result {
                    if total_cost < best_cost {
                        best_cost = total_cost;
                        best_result = Some(PlanResult {
                            success: true,
                            action_chain,
                            total_cost,
                            goal_index: goal_index as i64,
                            deferred_action_indices: vec![],
                            action_bindings: node.branch.action_bindings.clone(),
                        });
                        self.policy.on_valid_plan_found(best_cost, &self.stats);
                        self.stats.best_cost = best_cost;
                        self.stats.valid_plans_found += 1;
                    }
                }
                self.ctx.tree_dump.borrow_mut().set_outcome(
                    node.tree_node_id,
                    NodeOutcome::Complete {
                        chain_len: node.branch.action_chain.len(),
                        total_cost: node.branch.estimated_cost,
                        fwd_ok,
                    },
                );
                continue;
            }

            let expander = BranchExpander { ctx: self.ctx };
            let successors = expander.expand(&node);
            if successors.is_empty() {
                self.ctx
                    .tree_dump
                    .borrow_mut()
                    .set_outcome(node.tree_node_id, NodeOutcome::DeadEnd);
            }
            self.controller.push_successors(successors);
        }

        best_result
    }
}
