//! Backward-chaining GOAP planner operating on [`BlackboardSnapshot`]s.
//!
//! Searches backward from goals, selecting actions that satisfy open needs.
//! Uses requirements/provisions for symbolic dependency chaining and
//! simulate_effect for state-based goal progress. GDScript callables are
//! invoked indirectly via [`CallbackRequest`] / [`CallbackResponse`] channels.

use crate::plan_tree::PlanResult;
use crate::plan_types::*;
use crate::requirement::{ProvisionSpec, RequirementSpec, extract_initial_provisions};
use crate::snapshot::{BlackboardSnapshot, VariantSnapshot};
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
    /// Concrete provisions supplied by initial state and selected predecessor actions.
    bound_provisions: Vec<ProvisionSpec>,
    /// State-effect claims that require provider-bound re-simulation before completion.
    pending_effects: Vec<PendingEffectClaim>,
    /// Action-specific bindings: (action_index, fact_name, object_ids)
    /// Tracks which action each wildcard binding belongs to
    action_bindings: Vec<(i64, String, Vec<i64>)>,
    /// Accumulated lower bound cost estimate.
    estimated_cost: f64,
    /// State after all actions in this branch have been simulated forward.
    accumulated_agent: BlackboardSnapshot,
    accumulated_world: BlackboardSnapshot,
}

/// A state effect selected before all of its requirements were bound.
#[derive(Clone)]
struct PendingEffectClaim {
    action_idx: usize,
    preconditions: Vec<PreconditionSpec>,
}

/// Candidate action plus the open preconditions it can satisfy.
struct ActionCandidate {
    action_idx: usize,
    estimated_cost: f64,
    satisfied_precondition_indices: Vec<usize>,
    requires_bound_effect: bool,
}

impl PlanBranch {
    fn new(
        goal_preconditions: &[PreconditionSpec],
        initial_provisions: &[ProvisionSpec],
        initial_agent: &BlackboardSnapshot,
        initial_world: &BlackboardSnapshot,
    ) -> Self {
        Self {
            open_preconditions: goal_preconditions.to_vec(),
            open_requirements: vec![],
            action_chain: vec![],
            bound_provisions: initial_provisions.to_vec(),
            pending_effects: vec![],
            action_bindings: vec![],
            estimated_cost: 0.0,
            accumulated_agent: initial_agent.clone(),
            accumulated_world: initial_world.clone(),
        }
    }

    /// Returns true if all open needs are satisfied by the accumulated state and provisions.
    fn is_complete(
        &self,
        initial_provisions: &[ProvisionSpec],
        request_tx: &Sender<CallbackRequest>,
    ) -> bool {
        if !self.pending_effects.is_empty() {
            return false;
        }

        let preconditions_ok = self.open_preconditions.is_empty()
            || self.open_preconditions.iter().all(|p| {
                eval_precondition(
                    p,
                    &self.accumulated_agent,
                    &self.accumulated_world,
                    request_tx,
                )
            });

        let requirements_ok = self.open_requirements.is_empty()
            || requirements_satisfied_in_context(
                &self.open_requirements,
                initial_provisions,
                &self.accumulated_world,
            );

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
                action_bindings: vec![],
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
        let root_branch = PlanBranch::new(&goal.desired_state, &initial_provisions, &agent, &world);
        let mut best_cost = f64::INFINITY;
        let result = backward_search(root_branch, &ctx, 0, &mut best_cost);

        if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
            let _ = result_tx.send(None);
            return;
        }

        if let Some((action_chain, total_cost, action_bindings)) = result {
            crate::log_debug!(
                "Found valid plan for goal '{}' with cost {:.2}",
                goal.name,
                total_cost
            );
            let _ = result_tx.send(Some(PlanResult {
                success: true,
                action_chain,
                total_cost,
                goal_index: goal.original_index as i64,
                deferred_action_indices: vec![],
                action_bindings,
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
/// Returns (action_chain, total_cost, action_bindings) if a valid plan is found.
fn backward_search(
    branch: PlanBranch,
    ctx: &SearchContext,
    depth: usize,
    best_cost: &mut f64,
) -> Option<(Vec<i64>, f64, Vec<(i64, String, Vec<i64>)>)> {
    if ctx.cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
        return None;
    }

    if branch.estimated_cost >= *best_cost {
        crate::log_debug!(
            "Pruning branch at depth {} with estimated cost {:.2} (best valid: {:.2})",
            depth,
            branch.estimated_cost,
            *best_cost
        );
        return None;
    }

    if depth > ctx.max_depth {
        crate::log_debug!("Max depth {} reached", ctx.max_depth);
        return None;
    }

    // Check if branch is complete - all needs satisfied by accumulated state
    if branch.is_complete(ctx.initial_provisions, ctx.request_tx) {
        crate::log_debug!("Branch complete with {} actions", branch.action_chain.len());
        // Forward validate the complete chain with bindings to recalculate true costs
        let result = forward_validate(&branch.action_chain, &branch.action_bindings, ctx);
        if let Some((action_chain, total_cost)) = result
            && total_cost < *best_cost
        {
            *best_cost = total_cost;
            return Some((action_chain, total_cost, branch.action_bindings.clone()));
        }
        return None;
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

    let mut best_result: Option<(Vec<i64>, f64, Vec<(i64, String, Vec<i64>)>)> = None;

    // Try each candidate (already sorted by estimated cost)
    for candidate in candidates {
        if ctx.cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
            return None;
        }

        let action = &ctx.actions[candidate.action_idx];

        crate::log_debug!(
            "Trying action '{}' at depth {} (cost {:.2})",
            action.name,
            depth,
            candidate.estimated_cost
        );

        // Create new branch with this action as predecessor
        let mut new_branch = branch.clone();

        // Insert action at front of chain (execution order)
        new_branch
            .action_chain
            .insert(0, candidate.action_idx as i64);
        new_branch.estimated_cost += candidate.estimated_cost;

        // Update open needs: remove satisfied needs, add action's requirements/preconditions
        if !update_open_needs(&mut new_branch, &candidate, action, ctx) {
            crate::log_debug!(
                "Action '{}' could not resolve pending provider-bound effects",
                action.name
            );
            continue;
        }

        // Simulate this action's effect forward on the accumulated state
        // (must happen AFTER update_open_needs so requirements are resolved first)
        // Only simulate if the action's requirements are satisfied — otherwise
        // the effect callback may produce incorrect results (e.g. treating "" as
        // a valid binding when it should be null).
        let requirements_met = action.requirements.is_empty()
            || requirements_satisfied_in_context(
                &action.requirements,
                &new_branch.bound_provisions,
                &new_branch.accumulated_world,
            );
        if requirements_met {
            // If we have action-specific bindings, apply them to the blackboard temporarily
            // so the action's simulate_effect can use the concrete values
            let mut agent_for_sim = new_branch.accumulated_agent.clone();
            let world_for_sim = new_branch.accumulated_world.clone();

            // Look up bindings for this specific action
            for (action_idx, fact_name, object_ids) in &new_branch.action_bindings {
                if *action_idx == candidate.action_idx as i64 && !object_ids.is_empty() {
                    // Set the binding on the agent blackboard for the action to use
                    let id_variants: Vec<VariantSnapshot> = object_ids
                        .iter()
                        .map(|id| VariantSnapshot::ObjectRef(*id))
                        .collect();
                    let binding_value = VariantSnapshot::Array(id_variants);
                    agent_for_sim
                        .properties
                        .insert(fact_name.clone(), binding_value);
                }
            }

            let (after_agent, after_world) = call_apply_effect(
                action.effect_callable_id,
                agent_for_sim,
                world_for_sim,
                ctx.request_tx,
            );
            new_branch.accumulated_agent = after_agent;
            new_branch.accumulated_world = after_world;
        }

        // Recurse
        if let Some(result) = backward_search(new_branch, ctx, depth + 1, best_cost) {
            let should_update = best_result
                .as_ref()
                .map(|(_, best_cost, _)| result.1 < *best_cost)
                .unwrap_or(true);
            if should_update {
                crate::log_debug!(
                    "New best branch at depth {} with cost {:.2}",
                    depth,
                    result.1
                );
                best_result = Some(result);
            }
        }
    }

    best_result
}

/// Find actions that can satisfy at least one open need in the branch.
/// Returns vector of (action_index, estimated_cost) sorted by cost.
fn find_candidate_actions(branch: &PlanBranch, ctx: &SearchContext) -> Vec<ActionCandidate> {
    let mut candidates: Vec<ActionCandidate> = vec![];

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

        let action_candidates = action_candidates_for_needs(idx, action, branch, ctx);

        if !action_candidates.is_empty() {
            crate::log_debug!("Action '{}' can satisfy a need", action.name);

            candidates.extend(action_candidates);
        } else {
            crate::log_debug!("Action '{}' cannot satisfy any open need", action.name);
        }
    }

    crate::log_debug!("Found {} candidate actions", candidates.len());

    // Sort by estimated cost (lower bound), then by action index for deterministic ordering
    candidates.sort_by(|a, b| {
        a.estimated_cost
            .partial_cmp(&b.estimated_cost)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.action_idx.cmp(&b.action_idx))
    });

    candidates
}

/// Returns candidates for the needs an action can satisfy.
fn action_candidates_for_needs(
    action_idx: usize,
    action: &ActionSpec,
    branch: &PlanBranch,
    ctx: &SearchContext,
) -> Vec<ActionCandidate> {
    let mut candidates = Vec::new();

    // 1. Check if action's provisions satisfy any open requirement
    if !branch.open_requirements.is_empty() {
        // For wildcard provisions, evaluate cost separately for each requirement
        // to ensure the planner chooses the nearest target
        for req in &branch.open_requirements {
            for prov in &action.provisions {
                if provision_satisfies_requirement_in_context(prov, req, ctx.initial_world) {
                    // Extract binding that would be created if this provision satisfies this requirement
                    let binding = extract_binding_for_requirement(prov, req);
                    
                    crate::log_debug!(
                        "Action '{}' satisfies requirement via provision",
                        action.name
                    );
                    let estimated_cost = estimate_action_cost_with_binding(
                        action,
                        branch,
                        ctx,
                        &binding,
                    );
                    if estimated_cost != f64::INFINITY {
                        candidates.push(ActionCandidate {
                            action_idx,
                            estimated_cost,
                            satisfied_precondition_indices: vec![],
                            requires_bound_effect: false,
                        });
                    }
                    break;
                }
            }
        }
    }

    // 2. Check if action's effect can satisfy any open precondition
    if !branch.open_preconditions.is_empty() {
        let satisfied_indices = bound_effect_satisfied_precondition_indices(
            action,
            &branch.open_preconditions,
            &branch.bound_provisions,
            branch,
            ctx,
        );
        if !satisfied_indices.is_empty() {
            crate::log_debug!(
                "Action '{}' satisfies open preconditions via effect",
                action.name
            );
            let estimated_cost = estimate_action_cost(action, branch, ctx);
            if estimated_cost != f64::INFINITY {
                candidates.push(ActionCandidate {
                    action_idx,
                    estimated_cost,
                    satisfied_precondition_indices: satisfied_indices,
                    requires_bound_effect: false,
                });
            }
        } else if !action.requirements.is_empty() {
            let potential_satisfied_indices = potential_bound_effect_satisfied_precondition_indices(
                action,
                &branch.open_preconditions,
                &branch.bound_provisions,
                branch,
                ctx,
            );
            let estimated_cost = estimate_action_cost(action, branch, ctx);
            if estimated_cost != f64::INFINITY {
                for precondition_idx in potential_satisfied_indices {
                    candidates.push(ActionCandidate {
                        action_idx,
                        estimated_cost,
                        satisfied_precondition_indices: vec![precondition_idx],
                        requires_bound_effect: true,
                    });
                }
            }
        }
    }

    candidates
}

/// Returns indices of open preconditions satisfied by provider-bound simulation
/// starting from the branch's accumulated state.
fn bound_effect_satisfied_precondition_indices(
    action: &ActionSpec,
    open_preconditions: &[PreconditionSpec],
    bound_provisions: &[ProvisionSpec],
    branch: &PlanBranch,
    ctx: &SearchContext,
) -> Vec<usize> {
    let Some((hypo_agent, hypo_world)) = create_hypothetical_snapshot(
        &branch.accumulated_agent,
        &branch.accumulated_world,
        &action.requirements,
        bound_provisions,
    ) else {
        return vec![];
    };

    let before_satisfied: Vec<bool> = open_preconditions
        .iter()
        .map(|precond| precondition_satisfied(precond, &hypo_agent, &hypo_world, ctx))
        .collect();

    // Apply the action's effect
    let (after_agent, after_world) = call_apply_effect(
        action.effect_callable_id,
        hypo_agent,
        hypo_world,
        ctx.request_tx,
    );

    let mut satisfied_indices = Vec::new();
    for (idx, precond) in open_preconditions.iter().enumerate() {
        let after_satisfied = precondition_satisfied(precond, &after_agent, &after_world, ctx);
        if !before_satisfied[idx] && after_satisfied {
            satisfied_indices.push(idx);
        }
    }

    satisfied_indices
}

fn potential_bound_effect_satisfied_precondition_indices(
    action: &ActionSpec,
    open_preconditions: &[PreconditionSpec],
    bound_provisions: &[ProvisionSpec],
    branch: &PlanBranch,
    ctx: &SearchContext,
) -> Vec<usize> {
    let mut potential_provisions = bound_provisions.to_vec();
    for provider in ctx.actions {
        for provision in &provider.provisions {
            if !potential_provisions.contains(provision) {
                potential_provisions.push(provision.clone());
            }
        }
    }

    bound_effect_satisfied_precondition_indices(
        action,
        open_preconditions,
        &potential_provisions,
        branch,
        ctx,
    )
}

fn precondition_satisfied(
    precondition: &PreconditionSpec,
    agent: &BlackboardSnapshot,
    world: &BlackboardSnapshot,
    ctx: &SearchContext,
) -> bool {
    match precondition.evaluate_builtin(agent, world) {
        Some(result) => result,
        None => eval_precondition(precondition, agent, world, ctx.request_tx),
    }
}

/// Create a hypothetical snapshot from a base state and concrete bound provisions.
fn create_hypothetical_snapshot(
    base_agent: &BlackboardSnapshot,
    base_world: &BlackboardSnapshot,
    requirements: &[RequirementSpec],
    bound_provisions: &[ProvisionSpec],
) -> Option<(BlackboardSnapshot, BlackboardSnapshot)> {
    if !requirements_satisfied_in_context(requirements, bound_provisions, base_world) {
        return None;
    }

    let mut hypo_agent = base_agent.clone();
    let hypo_world = base_world.clone();

    for req in requirements {
        match req {
            RequirementSpec::BindingExists { binding_name } => {
                let value = bound_value_for_requirement(req, bound_provisions, base_world)?;
                hypo_agent.properties.insert(binding_name.clone(), value);
            }
            RequirementSpec::BindingEquals {
                binding_name,
                value,
            } => {
                hypo_agent
                    .properties
                    .insert(binding_name.clone(), value.clone());
            }
            RequirementSpec::BindingInSet { binding_name, .. } => {
                let value = bound_value_for_requirement(req, bound_provisions, base_world)?;
                hypo_agent.properties.insert(binding_name.clone(), value);
            }
            _ => {
                // Other requirement types - ignore for now
            }
        }
    }

    Some((hypo_agent, hypo_world))
}

/// Returns the concrete value from bound provisions that satisfies a binding requirement.
fn bound_value_for_requirement(
    requirement: &RequirementSpec,
    bound_provisions: &[ProvisionSpec],
    world: &BlackboardSnapshot,
) -> Option<VariantSnapshot> {
    bound_provisions
        .iter()
        .find_map(|provision| match provision {
            ProvisionSpec::Binding { value, .. }
                if provision_satisfies_requirement_in_context(provision, requirement, world) =>
            {
                Some(value.clone())
            }
            _ => None,
        })
}

/// Estimate action cost using hypothetical simulation from accumulated state.
fn estimate_action_cost(action: &ActionSpec, branch: &PlanBranch, ctx: &SearchContext) -> f64 {
    estimate_action_cost_with_binding(action, branch, ctx, &None)
}

/// Estimate action cost using hypothetical simulation with a specific binding applied.
fn estimate_action_cost_with_binding(
    action: &ActionSpec,
    branch: &PlanBranch,
    ctx: &SearchContext,
    binding: &Option<(String, Vec<i64>)>,
) -> f64 {
    let Some((hypo_agent, hypo_world)) = create_hypothetical_snapshot(
        &branch.accumulated_agent,
        &branch.accumulated_world,
        &action.requirements,
        &branch.bound_provisions,
    ) else {
        return 1.0;
    };

    // Apply the binding if provided (for wildcard provisions)
    let mut agent_for_cost = hypo_agent;
    if let Some((fact_name, object_ids)) = binding {
        let id_variants: Vec<VariantSnapshot> = object_ids
            .iter()
            .map(|id| VariantSnapshot::ObjectRef(*id))
            .collect();
        let binding_value = VariantSnapshot::Array(id_variants);
        agent_for_cost
            .properties
            .insert(fact_name.clone(), binding_value);
    }

    call_get_cost(
        action.cost_callable_id,
        &agent_for_cost,
        &hypo_world,
        ctx.request_tx,
    )
}

/// Extract the binding that would be created when a provision satisfies a requirement.
fn extract_binding_for_requirement(
    provision: &ProvisionSpec,
    requirement: &RequirementSpec,
) -> Option<(String, Vec<i64>)> {
    match (provision, requirement) {
        (
            ProvisionSpec::FactWildcard {
                fact_name: prov_name,
            },
            RequirementSpec::Fact {
                fact_name: req_name,
                args,
            },
        ) if prov_name == req_name => {
            let object_ids: Vec<i64> = args
                .iter()
                .filter_map(|v| {
                    if let VariantSnapshot::ObjectRef(id) = v {
                        Some(*id)
                    } else if let VariantSnapshot::Str(uid) = v {
                        uid.parse::<i64>().ok()
                    } else {
                        None
                    }
                })
                .collect();
            if !object_ids.is_empty() {
                Some((prov_name.clone(), object_ids))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Update open needs when adding a predecessor action.
/// This removes needs that the action satisfies and adds the action's own needs.
fn update_open_needs(
    branch: &mut PlanBranch,
    candidate: &ActionCandidate,
    action: &ActionSpec,
    ctx: &SearchContext,
) -> bool {
    // 1. Remove requirements satisfied by this action's provisions
    // Also capture wildcard bindings with action index
    let mut requirements_to_remove = Vec::new();
    let mut new_bindings = Vec::new();
    for (req_idx, req) in branch.open_requirements.iter().enumerate() {
        for prov in &action.provisions {
            if provision_satisfies_requirement_in_context(prov, req, ctx.initial_world) {
                requirements_to_remove.push(req_idx);
                // If this is a wildcard provision matching a specific fact requirement, capture the binding
                if let ProvisionSpec::FactWildcard {
                    fact_name: prov_name,
                } = prov
                {
                    if let RequirementSpec::Fact {
                        fact_name: req_name,
                        args,
                    } = req
                    {
                        if prov_name == req_name {
                            let object_ids: Vec<i64> = args
                                .iter()
                                .filter_map(|v| {
                                    if let VariantSnapshot::ObjectRef(id) = v {
                                        Some(*id)
                                    } else if let VariantSnapshot::Str(uid) = v {
                                        uid.parse::<i64>().ok()
                                    } else {
                                        None
                                    }
                                })
                                .collect();
                            if !object_ids.is_empty() {
                                // Track which action this binding belongs to
                                new_bindings.push((candidate.action_idx as i64, prov_name.clone(), object_ids));
                            }
                        }
                    }
                }
                break;
            }
        }
    }

    // Remove satisfied requirements (in reverse order to preserve indices)
    requirements_to_remove.sort();
    requirements_to_remove.reverse();
    for idx in requirements_to_remove {
        branch.open_requirements.remove(idx);
    }

    // Add new action-specific bindings
    for (action_idx, fact_name, object_ids) in new_bindings {
        branch.action_bindings.push((action_idx, fact_name, object_ids));
    }

    for prov in &action.provisions {
        if !branch.bound_provisions.contains(prov) {
            branch.bound_provisions.push(prov.clone());
        }
    }

    let claimed_preconditions = remove_preconditions_by_index(
        &mut branch.open_preconditions,
        &candidate.satisfied_precondition_indices,
    );

    if candidate.requires_bound_effect && !claimed_preconditions.is_empty() {
        branch.pending_effects.push(PendingEffectClaim {
            action_idx: candidate.action_idx,
            preconditions: claimed_preconditions,
        });
    }

    // 2. Add action's requirements as new open needs
    for req in &action.requirements {
        if !branch.open_requirements.contains(req) {
            branch.open_requirements.push(req.clone());
        }
    }

    // 3. Add action's preconditions as new open needs
    for precond in &action.preconditions {
        // Check if this precondition is already satisfied by accumulated state
        let already_satisfied = eval_precondition(
            precond,
            &branch.accumulated_agent,
            &branch.accumulated_world,
            ctx.request_tx,
        );

        if !already_satisfied {
            // Check for duplicates
            let is_duplicate = branch
                .open_preconditions
                .iter()
                .any(|p| preconditions_equal(p, precond));
            if !is_duplicate {
                branch.open_preconditions.push(precond.clone());
            }
        }
    }

    resolve_pending_effects(branch, ctx)
}

/// Removes preconditions by index and returns the removed entries.
fn remove_preconditions_by_index(
    preconditions: &mut Vec<PreconditionSpec>,
    indices: &[usize],
) -> Vec<PreconditionSpec> {
    let mut removed = Vec::new();
    let mut sorted_indices = indices.to_vec();
    sorted_indices.sort_unstable();
    sorted_indices.dedup();

    for idx in sorted_indices.into_iter().rev() {
        if idx < preconditions.len() {
            removed.push(preconditions.remove(idx));
        }
    }

    removed.reverse();
    removed
}

/// Re-simulates pending state-effect claims using currently bound concrete provisions.
fn resolve_pending_effects(branch: &mut PlanBranch, ctx: &SearchContext) -> bool {
    let mut unresolved_claims = Vec::new();
    let claims: Vec<_> = branch.pending_effects.drain(..).collect();

    for claim in claims {
        let action = &ctx.actions[claim.action_idx];
        let satisfied_indices = bound_effect_satisfied_precondition_indices(
            action,
            &claim.preconditions,
            &branch.bound_provisions,
            branch,
            ctx,
        );

        if satisfied_indices.len() == claim.preconditions.len() {
            continue;
        }

        let mut remaining_preconditions = claim.preconditions;
        remove_preconditions_by_index(&mut remaining_preconditions, &satisfied_indices);
        unresolved_claims.push(PendingEffectClaim {
            action_idx: claim.action_idx,
            preconditions: remaining_preconditions,
        });
    }

    branch.pending_effects = unresolved_claims;
    true
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
        ) => t1 == t2 && o1 == o2 && p1 == p2 && v1 == v2,
        _ => false,
    }
}

/// Validate a complete action chain by forward simulation.
/// Returns (action_chain, total_cost) if valid, None otherwise.
fn forward_validate(
    action_chain: &[i64],
    action_bindings: &[(i64, String, Vec<i64>)],
    ctx: &SearchContext,
) -> Option<(Vec<i64>, f64)> {
    let mut agent = ctx.initial_agent.clone();
    let mut world = ctx.initial_world.clone();
    let mut accumulated_provisions: Vec<ProvisionSpec> = ctx.initial_provisions.to_vec();
    let mut total_cost: f64 = 0.0;

    crate::log_debug!(
        "Forward validating chain with {} actions",
        action_chain.len()
    );

    for action_idx in action_chain {
        let action = &ctx.actions[*action_idx as usize];

        crate::log_debug!("Validating action '{}'", action.name);

        // Apply action-specific bindings before calculating cost
        // Bindings are associated with the action that provides the provision,
        // but they need to be applied to the agent blackboard for later actions
        // that have requirements satisfied by those provisions.
        for (binding_action_idx, fact_name, object_ids) in action_bindings {
            // Apply binding if this action is the one that provides the provision
            // This sets the binding on the agent for subsequent actions to use
            if *binding_action_idx == *action_idx && !object_ids.is_empty() {
                let id_variants: Vec<VariantSnapshot> = object_ids
                    .iter()
                    .map(|id| VariantSnapshot::ObjectRef(*id))
                    .collect();
                let binding_value = VariantSnapshot::Array(id_variants);
                agent
                    .properties
                    .insert(fact_name.clone(), binding_value);
                crate::log_debug!(
                    "Applied binding '{}' with {} objects (action_bindings) for action '{}'",
                    fact_name,
                    object_ids.len(),
                    action.name
                );
            }
        }

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
        if !requirements_satisfied_in_context(&action.requirements, &accumulated_provisions, &world)
        {
            crate::log_debug!(
                "Action '{}' failed: requirements not satisfied",
                action.name
            );
            return None;
        }

        // 4.5. For actions with wildcard provisions, if this action provides a binding
        // that satisfies a later action's requirement, apply it now so this action's cost
        // can be calculated with the true target location
        if action.provisions.iter().any(|p| matches!(p, ProvisionSpec::FactWildcard { .. })) {
            // Look for bindings where this action is the provider
            for (binding_action_idx, fact_name, object_ids) in action_bindings {
                if *binding_action_idx == *action_idx && !object_ids.is_empty() {
                    let id_variants: Vec<VariantSnapshot> = object_ids
                        .iter()
                        .map(|id| VariantSnapshot::ObjectRef(*id))
                        .collect();
                    let binding_value = VariantSnapshot::Array(id_variants);
                    agent
                        .properties
                        .insert(fact_name.clone(), binding_value);
                    crate::log_debug!(
                        "Applied binding '{}' with {} objects for cost calculation of '{}'",
                        fact_name,
                        object_ids.len(),
                        action.name
                    );
                }
            }
        }

        // 5. Get cost
        let cost = call_get_cost(action.cost_callable_id, &agent, &world, ctx.request_tx);
        crate::log_debug!("Action '{}' cost: {:.2}", action.name, cost);
        if cost == f64::INFINITY {
            crate::log_debug!("Action '{}' has infinite cost", action.name);
            return None;
        }
        total_cost += cost;

        // 6. Apply effect
        let (new_agent, new_world) =
            call_apply_effect(action.effect_callable_id, agent, world, ctx.request_tx);
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

    crate::log_debug!(
        "Forward validation succeeded, total cost: {:.2}",
        total_cost
    );

    Some((action_chain.to_vec(), total_cost))
}

/// Returns true when every requirement is satisfied by known provisions.
fn requirements_satisfied_in_context(
    requirements: &[RequirementSpec],
    provisions: &[ProvisionSpec],
    world: &BlackboardSnapshot,
) -> bool {
    requirements
        .iter()
        .all(|requirement| requirement_satisfied_in_context(requirement, provisions, world))
}

/// Returns true when any known provision satisfies a requirement.
fn requirement_satisfied_in_context(
    requirement: &RequirementSpec,
    provisions: &[ProvisionSpec],
    world: &BlackboardSnapshot,
) -> bool {
    provisions
        .iter()
        .any(|provision| provision_satisfies_requirement_in_context(provision, requirement, world))
}

/// Returns true when any provision satisfies at least one requirement.
fn provisions_satisfy_any_requirement_in_context(
    provisions: &[ProvisionSpec],
    requirements: &[RequirementSpec],
    world: &BlackboardSnapshot,
) -> bool {
    requirements
        .iter()
        .any(|requirement| requirement_satisfied_in_context(requirement, provisions, world))
}

/// Returns true when a provision satisfies a requirement with world-aware checks.
fn provision_satisfies_requirement_in_context(
    provision: &ProvisionSpec,
    requirement: &RequirementSpec,
    world: &BlackboardSnapshot,
) -> bool {
    match (provision, requirement) {
        (
            ProvisionSpec::Binding {
                binding_name: provided_name,
                value,
            },
            RequirementSpec::BindingExists { binding_name },
        ) => provided_name == binding_name && !value.is_null() && !value.is_empty_string(),
        (
            ProvisionSpec::Binding {
                binding_name: provided_name,
                value: provided_value,
            },
            RequirementSpec::BindingEquals {
                binding_name,
                value,
            },
        ) => provided_name == binding_name && provided_value == value,
        (
            ProvisionSpec::Binding {
                binding_name: provided_name,
                value,
            },
            RequirementSpec::BindingInSet {
                binding_name,
                set_name,
            },
        ) => provided_name == binding_name && binding_value_is_in_set(value, set_name, world),
        (
            ProvisionSpec::Fact {
                fact_name: provided_name,
                args: provided_args,
            },
            RequirementSpec::Fact { fact_name, args },
        ) => provided_name == fact_name && provided_args == args,
        (
            ProvisionSpec::FactWildcard {
                fact_name: provided_name,
            },
            RequirementSpec::Fact { fact_name, .. },
        ) => provided_name == fact_name,
        _ => false,
    }
}

/// Returns true when a binding value refers to a known world object in a group.
fn binding_value_is_in_set(
    value: &VariantSnapshot,
    set_name: &str,
    world: &BlackboardSnapshot,
) -> bool {
    match value {
        VariantSnapshot::ObjectRef(id) => world.objects.values().any(|object| {
            object.uid == id.to_string() && object.groups.contains(&set_name.to_string())
        }),
        VariantSnapshot::Str(uid) => world
            .objects
            .get(uid)
            .map(|object| object.groups.contains(&set_name.to_string()))
            .unwrap_or(false),
        _ => false,
    }
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
            if !result
                && let PreconditionSpec::Builtin {
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
