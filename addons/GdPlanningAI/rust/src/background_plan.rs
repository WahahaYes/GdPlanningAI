//! Background planning algorithm operating on [`BlackboardSnapshot`]s.
//!
//! This is the Send-safe counterpart of the synchronous search in
//! [`crate::planning_engine`]. GDScript callables are invoked indirectly
//! via [`CallbackRequest`] / [`CallbackResponse`] channels.

use crate::background_types::*;
use crate::plan_tree::{self, PlanResult, PlanTreeNode};
use crate::requirement::{
    extend_unique_requirements, provisions_satisfy_any_requirement, remove_satisfied_requirements,
    RequirementSpec,
};
use crate::snapshot::BlackboardSnapshot;
use std::sync::{Arc, atomic::AtomicBool};
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
    result_tx: Sender<Option<PlanResult>>,
    cancel_flag: Arc<AtomicBool>,
) {
    crate::log_debug!(
        "Starting background planning: {} actions, {} goals, max recursion {}",
        actions.len(),
        goals.len(),
        max_recursion
    );

    if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
        let _ = result_tx.send(None);
        return;
    }

    let mut sorted_goals = goals;
    sorted_goals.sort_by(|a, b| {
        b.reward
            .partial_cmp(&a.reward)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for goal in &sorted_goals {
        if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
            let _ = result_tx.send(None);
            return;
        }

        crate::log_debug!("Processing goal '{}' (reward: {:.1})", goal.name, goal.reward);

        if is_goal_satisfied(&goal.desired_state, &agent, &world, &request_tx) {
            crate::log_debug!("Goal '{}' already satisfied", goal.name);
            let _ = result_tx.send(Some(PlanResult {
                success: true,
                action_chain: vec![],
                total_cost: 0.0,
                goal_index: goal.original_index as i64,
            }));
            return;
        }

        let mut root_node = PlanTreeNode {
            action_index: -1,
            cost: 0.0,
            children: vec![],
        };

        let ctx = PlanContext {
            desired_state: &goal.desired_state,
            active_requirements: &[],
            actions: &actions,
            max_recursion,
            request_tx: &request_tx,
            cancel_flag: &cancel_flag,
        };

        let success = build_plan_recursive(&mut root_node, &agent, &world, 0, &ctx);

        if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
            let _ = result_tx.send(None);
            return;
        }

        if success {
            crate::log_debug!("Found valid plan for goal '{}'", goal.name);
            let plan = plan_tree::extract_best_plan(&root_node);
            let _ = result_tx.send(Some(PlanResult {
                success: true,
                action_chain: plan.actions,
                total_cost: plan.cost,
                goal_index: goal.original_index as i64,
            }));
            return;
        }
    }

    let _ = result_tx.send(Some(PlanResult::failure()));
}

// ---------------------------------------------------------------------------
// Recursive search (mirrors planning_engine::build_plan_recursive)
// ---------------------------------------------------------------------------

/// Immutable configuration shared across all recursion levels of the
/// background planner while searching for a plan for one goal.
struct PlanContext<'a> {
    desired_state: &'a [PreconditionSpec],
    active_requirements: &'a [RequirementSpec],
    actions: &'a [ActionSpec],
    max_recursion: usize,
    request_tx: &'a Sender<CallbackRequest>,
    cancel_flag: &'a Arc<AtomicBool>,
}

/// Captures a candidate branch discovered at the current recursion level.
///
/// The planner evaluates all viable actions for the level first, stores the
/// simulated successor state and accumulated desired preconditions here, then
/// descends into the pending branches in ascending lower-bound cost order.
struct PendingAction {
    action_index: i64,
    cost: f64,
    child_desired: Vec<PreconditionSpec>,
    child_requirements: Vec<RequirementSpec>,
    sim_agent: BlackboardSnapshot,
    sim_world: BlackboardSnapshot,
}

/// Returns the lowest cumulative cost among all completed plan leaves
/// reachable from `node`, including the accumulated `path_cost` leading
/// to that node.
fn current_best_plan_cost(node: &PlanTreeNode, path_cost: f64) -> f64 {
    let mut best_cost = f64::INFINITY;
    find_best_plan_cost(node, path_cost, &mut best_cost);
    best_cost
}

/// Depth-first helper for [`current_best_plan_cost`].
///
/// Only leaf nodes represent completed action chains, so interior nodes
/// contribute their own action cost and recurse into children.
fn find_best_plan_cost(node: &PlanTreeNode, path_cost: f64, best_cost: &mut f64) {
    let new_cost = path_cost + node.cost;

    if node.action_index >= 0 && node.children.is_empty() {
        *best_cost = (*best_cost).min(new_cost);
        return;
    }

    for child in &node.children {
        find_best_plan_cost(child, new_cost, best_cost);
    }
}

/// Core recursive background search step.
///
/// Iterates over all actions, skipping invalid or infinite-cost ones, and
/// records every viable branch for the current recursion level before any
/// descent occurs.
///
/// Actions that immediately satisfy the desired state are added as leaf
/// children right away. Remaining progress-making actions are stored as
/// pending branches, sorted by their current lower-bound cost, and then
/// explored recursively until the best completed chain is strictly cheaper
/// than every pending branch. Returns `true` if at least one satisfying
/// leaf was found.
fn build_plan_recursive(
    node: &mut PlanTreeNode,
    agent_state: &BlackboardSnapshot,
    world_state: &BlackboardSnapshot,
    recursion_level: usize,
    ctx: &PlanContext,
) -> bool {
    if ctx.cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
        return false;
    }

    if recursion_level > ctx.max_recursion {
        crate::log_debug!("Max recursion depth {} reached", ctx.max_recursion);
        return false;
    }

    crate::log_debug!("Recursion level {}, evaluating {} actions", recursion_level, ctx.actions.len());

    let mut has_solution = false;
    let mut pending_actions: Vec<PendingAction> = vec![];

    for (idx, action) in ctx.actions.iter().enumerate() {
        if ctx.cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
            return false;
        }

        crate::log_debug!("Evaluating action '{}' at depth {}", action.name, recursion_level);

        // Validity checks
        if !action_is_valid(action, agent_state, world_state, ctx.request_tx) {
            continue;
        }

        // Action preconditions
        if !action_preconditions_satisfied(action, agent_state, world_state, ctx.request_tx) {
            crate::log_debug!(
                "Action '{}' preconditions not satisfied at depth {}",
                action.name,
                recursion_level
            );
            continue;
        }

        // Clone snapshots for simulation (trivial — just HashMap clone)
        let mut sim_agent = agent_state.clone();
        let mut sim_world = world_state.clone();

        // Get cost via callback channel
        let cost = call_get_cost(
            action.cost_callable_id,
            &sim_agent,
            &sim_world,
            ctx.request_tx,
        );
        if cost == f64::INFINITY {
            crate::log_debug!("Action '{}' has infinite cost, skipping", action.name);
            continue;
        }

        crate::log_debug!("Action '{}' cost: {:.2}", action.name, cost);

        // Apply effect via callback channel — returns updated snapshots
        let (new_agent, new_world) = call_apply_effect(
            action.effect_callable_id,
            sim_agent,
            sim_world,
            ctx.request_tx,
        );
        sim_agent = new_agent;
        sim_world = new_world;

        // Check if this action makes progress toward the goal
        let makes_goal_progress =
            check_progress_toward_goal(ctx.desired_state, &sim_agent, &sim_world, ctx.request_tx);
        let satisfies_requirements =
            provisions_satisfy_any_requirement(&action.provisions, ctx.active_requirements);
        let introduces_requirements = !action.requirements.is_empty();

        if makes_goal_progress || satisfies_requirements || introduces_requirements {
            crate::log_debug!("Action '{}' contributes useful progress", action.name);

            let mut child_requirements =
                remove_satisfied_requirements(ctx.active_requirements, &action.provisions);
            extend_unique_requirements(&mut child_requirements, &action.requirements);

            if is_goal_satisfied(ctx.desired_state, &sim_agent, &sim_world, ctx.request_tx)
                && child_requirements.is_empty()
            {
                crate::log_debug!("Action '{}' satisfies goal immediately", action.name);
                let next_node = PlanTreeNode {
                    action_index: idx as i64,
                    cost,
                    children: vec![],
                };
                node.children.push(next_node);
                has_solution = true;
                continue;
            }

            let mut child_desired = ctx.desired_state.to_vec();
            child_desired.extend(action.preconditions.clone());

            pending_actions.push(PendingAction {
                action_index: idx as i64,
                cost,
                child_desired,
                child_requirements,
                sim_agent,
                sim_world,
            });
        } else {
            crate::log_debug!("Action '{}' does not make progress toward goal", action.name);
        }
    }

    pending_actions.sort_by(|a, b| {
        a.cost
            .partial_cmp(&b.cost)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    crate::log_debug!(
        "Sorted {} pending actions, exploring from lowest cost",
        pending_actions.len()
    );

    for pending in pending_actions {
        if ctx.cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
            return false;
        }

        let best_complete_cost = current_best_plan_cost(node, 0.0);
        if best_complete_cost < pending.cost {
            crate::log_debug!(
                "Pruning action with cost {:.2} (best complete: {:.2})",
                pending.cost,
                best_complete_cost
            );
            break;
        }

        crate::log_debug!(
            "Descending into recursion level {} with action cost {:.2}",
            recursion_level + 1,
            pending.cost
        );

        let mut next_node = PlanTreeNode {
            action_index: pending.action_index,
            cost: pending.cost,
            children: vec![],
        };

        let child_ctx = PlanContext {
            desired_state: &pending.child_desired,
            active_requirements: &pending.child_requirements,
            actions: ctx.actions,
            max_recursion: ctx.max_recursion,
            request_tx: ctx.request_tx,
            cancel_flag: ctx.cancel_flag,
        };

        if build_plan_recursive(
            &mut next_node,
            &pending.sim_agent,
            &pending.sim_world,
            recursion_level + 1,
            &child_ctx,
        ) {
            node.children.push(next_node);
            has_solution = true;
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

fn action_preconditions_satisfied(
    action: &ActionSpec,
    agent: &BlackboardSnapshot,
    world: &BlackboardSnapshot,
    request_tx: &Sender<CallbackRequest>,
) -> bool {
    action
        .preconditions
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
        crate::log_debug!(
            "Action '{}' validity check failed: dependent objects no longer valid",
            action.name
        );
        return false;
    }

    // Then check validity preconditions
    for (i, check) in action.validity_checks.iter().enumerate() {
        if !eval_precondition(check, agent, world, request_tx) {
            crate::log_debug!(
                "Action '{}' validity check {} failed",
                action.name,
                i
            );
            return false;
        }
    }
    true
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
        Some(result) => {
            if !result {
                if let PreconditionSpec::Builtin {
                    operation,
                    property_name,
                    ..
                } = spec
                {
                    crate::log_debug!(
                        "Builtin precondition failed: op={:?}, property='{}'",
                        operation,
                        property_name
                    );
                }
            }
            result
        }
        None => {
            // Custom callback — check dependencies first
            if !check_dependencies_valid(spec.dependent_object_ids()) {
                crate::log_debug!(
                    "Custom precondition failed: dependent objects no longer valid"
                );
                return false;
            }

            let callable_id = spec.callable_id().unwrap();
            let result = call_eval_custom_precond(callable_id, agent, world, request_tx);
            if !result {
                crate::log_debug!("Custom precondition callback returned false");
            }
            result
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
