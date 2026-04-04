//! Background planning algorithm operating on [`BlackboardSnapshot`]s.
//!
//! This is the Send-safe counterpart of the synchronous search in
//! [`crate::planning_engine`]. GDScript callables are invoked indirectly
//! via [`CallbackRequest`] / [`CallbackResponse`] channels.

use crate::background_types::*;
use crate::plan_tree::{self, PlanResult, PlanTreeNode};
use crate::snapshot::BlackboardSnapshot;
use std::sync::mpsc::Sender;

/// Entry point for background planning. Runs the full goal-prioritised
/// search and sends the result through `result_tx` when done.
pub fn run_plan(
    agent: BlackboardSnapshot,
    world: BlackboardSnapshot,
    actions: Vec<ActionSpec>,
    goals: Vec<GoalSpec>,
    max_recursion: usize,
    request_tx: Sender<CallbackRequest>,
    result_tx: Sender<PlanResult>,
) {
    let mut sorted_goals = goals;
    sorted_goals.sort_by(|a, b| {
        b.reward
            .partial_cmp(&a.reward)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for goal in &sorted_goals {
        if is_goal_satisfied(&goal.desired_state, &agent, &world, &request_tx) {
            let _ = result_tx.send(PlanResult {
                success: true,
                action_chain: vec![],
                total_cost: 0.0,
                goal_index: goal.original_index as i64,
            });
            return;
        }

        let mut root_node = PlanTreeNode {
            action_index: -1,
            cost: 0.0,
            children: vec![],
        };

        let success = build_plan_recursive(
            &mut root_node,
            &goal.desired_state,
            &agent,
            &world,
            &actions,
            0,
            max_recursion,
            &request_tx,
        );

        if success {
            let plan = plan_tree::extract_best_plan(&root_node);
            let _ = result_tx.send(PlanResult {
                success: true,
                action_chain: plan.actions,
                total_cost: plan.cost,
                goal_index: goal.original_index as i64,
            });
            return;
        }
    }

    let _ = result_tx.send(PlanResult::failure());
}

// ---------------------------------------------------------------------------
// Recursive search (mirrors planning_engine::build_plan_recursive)
// ---------------------------------------------------------------------------

fn build_plan_recursive(
    node: &mut PlanTreeNode,
    desired_state: &[PreconditionSpec],
    agent_state: &BlackboardSnapshot,
    world_state: &BlackboardSnapshot,
    actions: &[ActionSpec],
    recursion_level: usize,
    max_recursion: usize,
    request_tx: &Sender<CallbackRequest>,
) -> bool {
    if recursion_level > max_recursion {
        return false;
    }

    let mut has_solution = false;

    for (idx, action) in actions.iter().enumerate() {
        // Validity checks
        if !action_is_valid(action, agent_state, world_state, request_tx) {
            continue;
        }

        // Clone snapshots for simulation (trivial — just HashMap clone)
        let mut sim_agent = agent_state.clone();
        let mut sim_world = world_state.clone();

        // Get cost via callback channel
        let cost = call_get_cost(action.cost_callable_id, &sim_agent, &sim_world, request_tx);
        if cost == f64::INFINITY {
            continue;
        }

        // Apply effect via callback channel — returns updated snapshots
        let (new_agent, new_world) =
            call_apply_effect(action.effect_callable_id, sim_agent, sim_world, request_tx);
        sim_agent = new_agent;
        sim_world = new_world;

        // Check if this action makes progress toward the goal
        let makes_progress =
            check_progress_toward_goal(desired_state, &sim_agent, &sim_world, request_tx);

        if makes_progress {
            if is_goal_satisfied(desired_state, &sim_agent, &sim_world, request_tx) {
                let next_node = PlanTreeNode {
                    action_index: idx as i64,
                    cost,
                    children: vec![],
                };
                node.children.push(next_node);
                has_solution = true;
                continue;
            }

            let mut next_node = PlanTreeNode {
                action_index: idx as i64,
                cost,
                children: vec![],
            };

            // Propagate action preconditions as additional constraints
            let mut child_desired = desired_state.to_vec();
            child_desired.extend(action.preconditions.clone());

            if build_plan_recursive(
                &mut next_node,
                &child_desired,
                &sim_agent,
                &sim_world,
                actions,
                recursion_level + 1,
                max_recursion,
                request_tx,
            ) {
                node.children.push(next_node);
                has_solution = true;
            }
        }
    }

    has_solution
}

// ---------------------------------------------------------------------------
// Precondition evaluation helpers
// ---------------------------------------------------------------------------

fn check_progress_toward_goal(
    preconditions: &[PreconditionSpec],
    agent: &BlackboardSnapshot,
    world: &BlackboardSnapshot,
    request_tx: &Sender<CallbackRequest>,
) -> bool {
    for precond in preconditions {
        if eval_precondition(precond, agent, world, request_tx) {
            return true;
        }
    }
    false
}

fn is_goal_satisfied(
    preconditions: &[PreconditionSpec],
    agent: &BlackboardSnapshot,
    world: &BlackboardSnapshot,
    request_tx: &Sender<CallbackRequest>,
) -> bool {
    preconditions
        .iter()
        .all(|p| eval_precondition(p, agent, world, request_tx))
}

fn action_is_valid(
    action: &ActionSpec,
    agent: &BlackboardSnapshot,
    world: &BlackboardSnapshot,
    request_tx: &Sender<CallbackRequest>,
) -> bool {
    // First check if all dependent objects still exist
    if !check_dependencies_valid(&action.dependent_object_ids) {
        return false;
    }

    // Then check validity preconditions
    action
        .validity_checks
        .iter()
        .all(|check| eval_precondition(check, agent, world, request_tx))
}

/// Validate that all dependent object IDs still refer to live objects.
/// This prevents attempting to invoke callables whose target objects have been freed.
fn check_dependencies_valid(dependent_ids: &[i64]) -> bool {
    use godot::obj::InstanceId;
    use godot::prelude::Gd;

    dependent_ids.iter().all(|id| {
        let instance_id = InstanceId::from_i64(*id);
        Gd::<godot::prelude::Object>::try_from_instance_id(instance_id).is_ok()
    })
}

fn eval_precondition(
    spec: &PreconditionSpec,
    agent: &BlackboardSnapshot,
    world: &BlackboardSnapshot,
    request_tx: &Sender<CallbackRequest>,
) -> bool {
    match spec.evaluate_builtin(agent, world) {
        Some(result) => result,
        None => {
            // Custom callback — check dependencies first
            if !check_dependencies_valid(spec.dependent_object_ids()) {
                return false;
            }

            let callable_id = spec.callable_id().unwrap();
            call_eval_custom_precond(callable_id, agent, world, request_tx)
        }
    }
}

// ---------------------------------------------------------------------------
// Channel callback helpers
// ---------------------------------------------------------------------------

fn call_get_cost(
    callable_id: usize,
    agent: &BlackboardSnapshot,
    world: &BlackboardSnapshot,
    request_tx: &Sender<CallbackRequest>,
) -> f64 {
    let (resp_tx, resp_rx) = std::sync::mpsc::channel();
    let _ = request_tx.send(CallbackRequest {
        callable_id,
        kind: CallbackKind::GetCost {
            agent: agent.clone(),
            world: world.clone(),
        },
        response_tx: resp_tx,
    });
    match resp_rx.recv() {
        Ok(CallbackResponse::Float(f)) => f,
        _ => f64::INFINITY,
    }
}

fn call_apply_effect(
    callable_id: usize,
    agent: BlackboardSnapshot,
    world: BlackboardSnapshot,
    request_tx: &Sender<CallbackRequest>,
) -> (BlackboardSnapshot, BlackboardSnapshot) {
    let (resp_tx, resp_rx) = std::sync::mpsc::channel();
    let _ = request_tx.send(CallbackRequest {
        callable_id,
        kind: CallbackKind::ApplyEffect { agent, world },
        response_tx: resp_tx,
    });
    match resp_rx.recv() {
        Ok(CallbackResponse::UpdatedSnapshots(a, w)) => (a, w),
        _ => {
            // If the channel is broken we can't recover — return empty snapshots
            (
                BlackboardSnapshot {
                    properties: Default::default(),
                    objects: Default::default(),
                },
                BlackboardSnapshot {
                    properties: Default::default(),
                    objects: Default::default(),
                },
            )
        }
    }
}

fn call_eval_custom_precond(
    callable_id: usize,
    agent: &BlackboardSnapshot,
    world: &BlackboardSnapshot,
    request_tx: &Sender<CallbackRequest>,
) -> bool {
    let (resp_tx, resp_rx) = std::sync::mpsc::channel();
    let _ = request_tx.send(CallbackRequest {
        callable_id,
        kind: CallbackKind::EvalCustomPrecond {
            agent: agent.clone(),
            world: world.clone(),
        },
        response_tx: resp_tx,
    });
    matches!(resp_rx.recv(), Ok(CallbackResponse::Bool(true)))
}
