//! Backward-chaining GOAP planner operating on [`BlackboardSnapshot`]s.
//!
//! Searches backward from goals, selecting actions that satisfy open needs.
//! Uses requirements/provisions for symbolic dependency chaining and
//! simulate_effect for state-based goal progress. GDScript callables are
//! invoked indirectly via [`CallbackRequest`] / [`CallbackResponse`] channels.

use crate::plan_tree::PlanResult;
use crate::plan_types::*;
use crate::requirement::{
    extract_initial_provisions, provision_satisfies_requirement, provisions_satisfy_any_requirement,
    requirements_satisfied, ProvisionSpec, RequirementSpec,
};
use crate::snapshot::BlackboardSnapshot;
use std::sync::mpsc::Sender;
use std::sync::{Arc, atomic::AtomicBool};

/// A branch in the backward search represents a partial plan suffix and
/// the set of unresolved needs that must be satisfied before it can execute.
#[derive(Clone)]
struct PlanBranch {
    /// Precondition specs that must be satisfied (state needs).
    open_preconditions: Vec<PreconditionSpec>,
    /// Requirement specs that must be satisfied (binding/fact needs).
    open_requirements: Vec<RequirementSpec>,
    /// Action indices in execution order (first = execute first).
    action_chain: Vec<i64>,
    /// Accumulated lower bound cost estimate.
    estimated_cost: f64,
}

impl PlanBranch {
    fn new(goal_preconditions: &[PreconditionSpec]) -> Self {
        Self {
            open_preconditions: goal_preconditions.to_vec(),
            open_requirements: vec![],
            action_chain: vec![],
            estimated_cost: 0.0,
        }
    }

    /// Returns true if all open needs are satisfied by the given initial state and provisions.
    fn is_complete(&self, agent: &BlackboardSnapshot, world: &BlackboardSnapshot, initial_provisions: &[ProvisionSpec]) -> bool {
        // Check if all preconditions are satisfied
        let preconditions_ok = self.open_preconditions.is_empty()
            || self.open_preconditions.iter().all(|p| {
                p.evaluate_builtin(agent, world).unwrap_or(false)
            });

        // Check if all requirements are satisfied
        let requirements_ok = self.open_requirements.is_empty()
            || requirements_satisfied(&self.open_requirements, initial_provisions);

        preconditions_ok && requirements_ok
    }
}

/// Entry point for planning. Runs the full goal-prioritised
/// backward search and sends the result through `result_tx` when done.
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
        "Starting backward planning: {} actions, {} goals, max recursion {}",
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

        crate::log_debug!(
            "Processing goal '{}' (reward: {:.1})",
            goal.name,
            goal.reward
        );

        // Check if goal is already satisfied
        let goal_satisfied = goal.desired_state.iter().all(|p| {
            p.evaluate_builtin(&agent, &world).unwrap_or_else(|| {
                // Custom precondition - check via callback
                eval_precondition(p, &agent, &world, &request_tx)
            })
        });

        if goal_satisfied {
            crate::log_debug!("Goal '{}' already satisfied", goal.name);
            let _ = result_tx.send(Some(PlanResult {
                success: true,
                action_chain: vec![],
                total_cost: 0.0,
                goal_index: goal.original_index as i64,
                deferred_action_indices: vec![],
            }));
            return;
        }

        // Extract initial provisions from agent state
        let initial_provisions = extract_initial_provisions(&agent);

        let ctx = SearchContext {
            actions: &actions,
            initial_agent: &agent,
            initial_world: &world,
            initial_provisions: &initial_provisions,
            goal_preconditions: &goal.desired_state,
            max_depth: max_recursion,
            request_tx: &request_tx,
            cancel_flag: &cancel_flag,
        };

        // Start backward search from goal
        let root_branch = PlanBranch::new(&goal.desired_state);
        let result = backward_search(root_branch, &ctx, 0);

        if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
            let _ = result_tx.send(None);
            return;
        }

        if let Some((action_chain, total_cost)) = result {
            crate::log_debug!("Found valid plan for goal '{}' with cost {:.2}", goal.name, total_cost);
            let _ = result_tx.send(Some(PlanResult {
                success: true,
                action_chain,
                total_cost,
                goal_index: goal.original_index as i64,
                deferred_action_indices: vec![],
            }));
            return;
        }
    }

    let _ = result_tx.send(Some(PlanResult::failure()));
}

/// Immutable configuration shared across all recursion levels.
struct SearchContext<'a> {
    actions: &'a [ActionSpec],
    initial_agent: &'a BlackboardSnapshot,
    initial_world: &'a BlackboardSnapshot,
    initial_provisions: &'a [ProvisionSpec],
    goal_preconditions: &'a [PreconditionSpec],
    max_depth: usize,
    request_tx: &'a Sender<CallbackRequest>,
    cancel_flag: &'a Arc<AtomicBool>,
}

/// Core backward chaining search.
/// Returns (action_chain, total_cost) if a valid plan is found.
fn backward_search(
    branch: PlanBranch,
    ctx: &SearchContext,
    depth: usize,
) -> Option<(Vec<i64>, f64)> {
    if ctx.cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
        return None;
    }

    if depth > ctx.max_depth {
        crate::log_debug!("Max depth {} reached", ctx.max_depth);
        return None;
    }

    // Check if branch is complete - all needs satisfied by initial state
    if branch.is_complete(ctx.initial_agent, ctx.initial_world, ctx.initial_provisions) {
        crate::log_debug!("Branch complete with {} actions", branch.action_chain.len());
        // Forward validate the complete chain
        return forward_validate(&branch.action_chain, ctx);
    }

    // Find candidate actions that can satisfy an open need
    let candidates = find_candidate_actions(&branch, ctx);

    if candidates.is_empty() {
        crate::log_debug!("No candidate actions found at depth {}", depth);
        return None;
    }

    crate::log_debug!(
        "Depth {}: {} candidates for {} open preconditions, {} open requirements",
        depth,
        candidates.len(),
        branch.open_preconditions.len(),
        branch.open_requirements.len()
    );

    // Try each candidate (already sorted by estimated cost)
    for (action_idx, estimated_cost) in candidates {
        if ctx.cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
            return None;
        }

        let action = &ctx.actions[action_idx];

        crate::log_debug!(
            "Trying action '{}' at depth {} (cost {:.2})",
            action.name,
            depth,
            estimated_cost
        );

        // Create new branch with this action as predecessor
        let mut new_branch = branch.clone();

        // Insert action at front of chain (execution order)
        new_branch.action_chain.insert(0, action_idx as i64);
        new_branch.estimated_cost += estimated_cost;

        // Update open needs: remove satisfied needs, add action's requirements/preconditions
        update_open_needs(&mut new_branch, action, ctx);

        // Recurse
        if let Some(result) = backward_search(new_branch, ctx, depth + 1) {
            return Some(result);
        }
    }

    None
}

/// Find actions that can satisfy at least one open need in the branch.
/// Returns vector of (action_index, estimated_cost) sorted by cost.
fn find_candidate_actions(
    branch: &PlanBranch,
    ctx: &SearchContext,
) -> Vec<(usize, f64)> {
    let mut candidates: Vec<(usize, f64)> = vec![];

    crate::log_debug!(
        "Finding candidates among {} actions for {} preconditions and {} requirements",
        ctx.actions.len(),
        branch.open_preconditions.len(),
        branch.open_requirements.len()
    );

    for (idx, action) in ctx.actions.iter().enumerate() {
        // Check if action is valid (dependencies exist)
        if !action_is_valid(action, ctx) {
            crate::log_debug!("Action '{}' invalid (dependencies)", action.name);
            continue;
        }

        // Check if action can satisfy any open need
        let can_satisfy = can_action_satisfy_need(action, branch, ctx);

        if can_satisfy {
            crate::log_debug!("Action '{}' can satisfy a need", action.name);
            // Get estimated cost via hypothetical simulation
            let cost = estimate_action_cost(action, branch, ctx);
            crate::log_debug!("Action '{}' estimated cost: {}", action.name, cost);

            if cost != f64::INFINITY {
                candidates.push((idx, cost));
            }
        } else {
            crate::log_debug!("Action '{}' cannot satisfy any open need", action.name);
        }
    }

    crate::log_debug!("Found {} candidate actions", candidates.len());

    // Sort by estimated cost (lower bound), then by action index for deterministic ordering
    candidates.sort_by(|a, b| {
        a.1.partial_cmp(&b.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });

    candidates
}

/// Check if an action can satisfy at least one open need in the branch.
fn can_action_satisfy_need(
    action: &ActionSpec,
    branch: &PlanBranch,
    ctx: &SearchContext,
) -> bool {
    // 1. Check if action's provisions satisfy any open requirement
    if !branch.open_requirements.is_empty() {
        let satisfies_req = provisions_satisfy_any_requirement(&action.provisions, &branch.open_requirements);
        if satisfies_req {
            crate::log_debug!("Action '{}' satisfies open requirements via provisions", action.name);
            return true;
        }
    }

    // 2. Check if action's effect can satisfy any open precondition
    // Use hypothetical simulation: create a snapshot with action's requirements satisfied
    if !branch.open_preconditions.is_empty() {
        let satisfies_precond = can_effect_satisfy_precondition(action, &branch.open_preconditions, ctx);
        if satisfies_precond {
            crate::log_debug!("Action '{}' satisfies open preconditions via effect", action.name);
            return true;
        }
    }

    false
}

/// Check if action's effect can satisfy any of the open preconditions.
/// Uses hypothetical simulation with requirements satisfied.
fn can_effect_satisfy_precondition(
    action: &ActionSpec,
    open_preconditions: &[PreconditionSpec],
    ctx: &SearchContext,
) -> bool {
    // Create hypothetical snapshot where action's requirements are satisfied
    let (hypo_agent, hypo_world) = create_hypothetical_snapshot(
        ctx.initial_agent,
        ctx.initial_world,
        &action.requirements,
    );

    // Apply the action's effect
    let (after_agent, after_world) = call_apply_effect(
        action.effect_callable_id,
        hypo_agent,
        hypo_world,
        ctx.request_tx,
    );

    // Check if any open precondition is now satisfied
    for precond in open_preconditions {
        let result = precond.evaluate_builtin(&after_agent, &after_world);
        if let Some(true) = result {
            return true;
        }
        // For custom preconditions, try evaluating
        if result.is_none() {
            if eval_precondition(precond, &after_agent, &after_world, ctx.request_tx) {
                return true;
            }
        }
    }

    false
}

/// Create a hypothetical snapshot with requirements satisfied for simulation.
fn create_hypothetical_snapshot(
    agent: &BlackboardSnapshot,
    world: &BlackboardSnapshot,
    requirements: &[RequirementSpec],
) -> (BlackboardSnapshot, BlackboardSnapshot) {
    let mut hypo_agent = agent.clone();
    let hypo_world = world.clone();

    for req in requirements {
        match req {
            RequirementSpec::BindingExists { binding_name } => {
                // Set a placeholder string value for the binding
                // Using a string ensures compatibility with GDScript string comparisons
                hypo_agent
                    .properties
                    .insert(binding_name.clone(), crate::snapshot::VariantSnapshot::Str("hypothetical".to_string()));
            }
            RequirementSpec::BindingEquals { binding_name, value } => {
                hypo_agent.properties.insert(binding_name.clone(), value.clone());
            }
            _ => {
                // Other requirement types - ignore for now
            }
        }
    }

    (hypo_agent, hypo_world)
}

/// Estimate action cost using hypothetical simulation.
fn estimate_action_cost(
    action: &ActionSpec,
    _branch: &PlanBranch,
    ctx: &SearchContext,
) -> f64 {
    // Create hypothetical snapshot where requirements are satisfied
    let (hypo_agent, hypo_world) = create_hypothetical_snapshot(
        ctx.initial_agent,
        ctx.initial_world,
        &action.requirements,
    );

    call_get_cost(action.cost_callable_id, &hypo_agent, &hypo_world, ctx.request_tx)
}

/// Update open needs when adding a predecessor action.
/// This removes needs that the action satisfies and adds the action's own needs.
fn update_open_needs(branch: &mut PlanBranch, action: &ActionSpec, ctx: &SearchContext) {
    // Create hypothetical snapshot to test what this action's effect satisfies
    let (hypo_agent, hypo_world) = create_hypothetical_snapshot(
        ctx.initial_agent,
        ctx.initial_world,
        &action.requirements,
    );
    let (after_agent, after_world) = call_apply_effect(
        action.effect_callable_id,
        hypo_agent,
        hypo_world,
        ctx.request_tx,
    );

    // 1. Remove requirements satisfied by this action's provisions
    branch.open_requirements.retain(|req| {
        !action.provisions.iter().any(|prov| provision_satisfies_requirement(prov, req))
    });

    // 2. Remove preconditions that this action's effect satisfies
    branch.open_preconditions.retain(|precond| {
        // Check if precondition is now satisfied after the action's effect
        let builtin_result = precond.evaluate_builtin(&after_agent, &after_world);
        let satisfied = match builtin_result {
            Some(result) => result,
            None => eval_precondition(precond, &after_agent, &after_world, ctx.request_tx),
        };
        !satisfied
    });

    // 3. Add action's requirements as new open needs
    for req in &action.requirements {
        if !branch.open_requirements.contains(req) {
            branch.open_requirements.push(req.clone());
        }
    }

    // 4. Add action's preconditions as new open needs
    for precond in &action.preconditions {
        // Check if this precondition is already satisfied by initial state
        let already_satisfied = precond.evaluate_builtin(
            ctx.initial_agent,
            ctx.initial_world,
        ).unwrap_or(false);

        if !already_satisfied {
            // Check for duplicates
            let is_duplicate = branch.open_preconditions.iter().any(|p| preconditions_equal(p, precond));
            if !is_duplicate {
                branch.open_preconditions.push(precond.clone());
            }
        }
    }
}

/// Check if two preconditions are equal.
fn preconditions_equal(a: &PreconditionSpec, b: &PreconditionSpec) -> bool {
    match (a, b) {
        (
            PreconditionSpec::Builtin {
                target: t1,
                operation: o1,
                property_name: p1,
                value: v1,
            },
            PreconditionSpec::Builtin {
                target: t2,
                operation: o2,
                property_name: p2,
                value: v2,
            },
        ) => {
            t1 == t2 && o1 == o2 && p1 == p2 && v1 == v2
        }
        _ => false,
    }
}

/// Validate a complete action chain by forward simulation.
/// Returns (action_chain, total_cost) if valid, None otherwise.
fn forward_validate(
    action_chain: &[i64],
    ctx: &SearchContext,
) -> Option<(Vec<i64>, f64)> {
    let mut agent = ctx.initial_agent.clone();
    let mut world = ctx.initial_world.clone();
    let mut accumulated_provisions: Vec<ProvisionSpec> = ctx.initial_provisions.to_vec();
    let mut total_cost: f64 = 0.0;

    crate::log_debug!("Forward validating chain with {} actions", action_chain.len());

    for action_idx in action_chain {
        let action = &ctx.actions[*action_idx as usize];

        crate::log_debug!("Validating action '{}'", action.name);

        // 1. Check dependencies valid
        if !check_dependencies_valid(&action.dependent_object_ids) {
            crate::log_debug!("Action '{}' failed: dependencies invalid", action.name);
            return None;
        }

        // 2. Check validity checks
        for (i, check) in action.validity_checks.iter().enumerate() {
            if !eval_precondition(check, &agent, &world, ctx.request_tx) {
                crate::log_debug!("Action '{}' failed validity check {}", action.name, i);
                return None;
            }
        }

        // 3. Check preconditions
        for precond in &action.preconditions {
            if !eval_precondition(precond, &agent, &world, ctx.request_tx) {
                crate::log_debug!("Action '{}' failed precondition", action.name);
                return None;
            }
        }

        // 4. Check requirements satisfied by accumulated provisions
        if !requirements_satisfied(&action.requirements, &accumulated_provisions) {
            crate::log_debug!("Action '{}' failed: requirements not satisfied", action.name);
            return None;
        }

        // 5. Get cost
        let cost = call_get_cost(action.cost_callable_id, &agent, &world, ctx.request_tx);
        if cost == f64::INFINITY {
            crate::log_debug!("Action '{}' has infinite cost", action.name);
            return None;
        }
        total_cost += cost;

        // 6. Apply effect
        let (new_agent, new_world) = call_apply_effect(
            action.effect_callable_id,
            agent,
            world,
            ctx.request_tx,
        );
        agent = new_agent;
        world = new_world;

        // 7. Accumulate provisions
        for prov in &action.provisions {
            if !accumulated_provisions.contains(prov) {
                accumulated_provisions.push(prov.clone());
            }
        }
    }

    if !ctx
        .goal_preconditions
        .iter()
        .all(|precond| eval_precondition(precond, &agent, &world, ctx.request_tx))
    {
        crate::log_debug!("Forward validation failed: final goal not satisfied");
        return None;
    }

    crate::log_debug!("Forward validation succeeded, total cost: {:.2}", total_cost);
    Some((action_chain.to_vec(), total_cost))
}

/// Check if action is valid (dependencies exist).
fn action_is_valid(action: &ActionSpec, _ctx: &SearchContext) -> bool {
    check_dependencies_valid(&action.dependent_object_ids)
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
                crate::log_debug!("Custom precondition failed: dependent objects no longer valid");
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
