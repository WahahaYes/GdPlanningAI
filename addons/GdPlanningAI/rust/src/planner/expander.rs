use crate::debug_tree::{NodeOutcome, TreeDump};
use crate::plan_types::*;
use crate::precondition::PreconditionTarget;
use crate::requirement::{ProvisionSpec, RequirementSpec};
use crate::snapshot::{BlackboardSnapshot, VariantSnapshot};
use std::cell::RefCell;
use std::sync::mpsc::Sender;
use std::sync::{Arc, atomic::AtomicBool};

use super::controller::SearchNode;
use super::heuristic;

#[derive(Clone)]
pub struct PlanBranch {
    pub open_preconditions: Vec<PreconditionSpec>,
    pub open_requirements: Vec<RequirementSpec>,
    pub open_requirement_consumers: Vec<i64>,
    pub action_chain: Vec<i64>,
    pub bound_provisions: Vec<ProvisionSpec>,
    pub pending_effects: Vec<PendingEffectClaim>,
    pub action_bindings: Vec<(i64, String, Vec<i64>)>,
    pub estimated_cost: f64,
    pub accumulated_agent: BlackboardSnapshot,
    pub accumulated_world: BlackboardSnapshot,
}

#[derive(Clone)]
pub struct PendingEffectClaim {
    pub action_idx: usize,
    pub preconditions: Vec<PreconditionSpec>,
}

pub struct ActionCandidate {
    pub action_idx: usize,
    pub estimated_cost: f64,
    pub satisfied_precondition_indices: Vec<usize>,
    pub satisfied_requirement_indices: Vec<usize>,
    pub requires_bound_effect: bool,
}

impl PlanBranch {
    pub fn new(
        goal_preconditions: &[PreconditionSpec],
        initial_provisions: &[ProvisionSpec],
        initial_agent: &BlackboardSnapshot,
        initial_world: &BlackboardSnapshot,
    ) -> Self {
        Self {
            open_preconditions: goal_preconditions.to_vec(),
            open_requirements: vec![],
            open_requirement_consumers: vec![],
            action_chain: vec![],
            bound_provisions: initial_provisions.to_vec(),
            pending_effects: vec![],
            action_bindings: vec![],
            estimated_cost: 0.0,
            accumulated_agent: initial_agent.clone(),
            accumulated_world: initial_world.clone(),
        }
    }

    pub fn is_complete(&self, request_tx: &Sender<CallbackRequest>) -> bool {
        if !self.pending_effects.is_empty() {
            return false;
        }

        let preconditions_ok = self.open_preconditions.is_empty()
            || self.open_preconditions.iter().all(|p| {
                super::eval_precondition(
                    p,
                    &self.accumulated_agent,
                    &self.accumulated_world,
                    request_tx,
                )
            });

        let requirements_ok = self.open_requirements.is_empty()
            || super::requirements_satisfied_in_context(
                &self.open_requirements,
                &self.bound_provisions,
                &self.accumulated_world,
            );

        preconditions_ok && requirements_ok
    }
}

pub struct BranchExpander<'ctx> {
    pub ctx: &'ctx SearchContext<'ctx>,
}

impl<'ctx> BranchExpander<'ctx> {
    pub fn expand(&self, node: &SearchNode) -> Vec<SearchNode> {
        if node.branch.is_complete(self.ctx.request_tx) {
            return vec![];
        }

        let candidates = find_candidate_actions(&node.branch, self.ctx);
        if candidates.is_empty() {
            crate::log_debug!("No candidate actions found at depth {}", node.depth);
            return vec![];
        }

        crate::log_debug!(
            "Depth {}: {} candidates for {} open preconditions, {} open requirements",
            node.depth,
            candidates.len(),
            node.branch.open_preconditions.len(),
            node.branch.open_requirements.len()
        );

        let mut successors = Vec::with_capacity(candidates.len());

        for candidate in &candidates {
            let action = &self.ctx.actions[candidate.action_idx];

            crate::log_debug!(
                "Trying action '{}' at depth {} (cost {:.2})",
                action.name,
                node.depth,
                candidate.estimated_cost
            );

            let mut new_branch = node.branch.clone();

            let insert_pos = insertion_index_for_candidate(&new_branch, candidate);
            shift_branch_positions_for_insert(&mut new_branch, insert_pos);
            new_branch
                .action_chain
                .insert(insert_pos, candidate.action_idx as i64);
            new_branch.estimated_cost += candidate.estimated_cost;

            if !update_open_needs(&mut new_branch, candidate, action, insert_pos, self.ctx) {
                continue;
            }

            let requirements_met = action.requirements.is_empty()
                || super::requirements_satisfied_in_context(
                    &action.requirements,
                    &new_branch.bound_provisions,
                    &new_branch.accumulated_world,
                );
            if requirements_met {
                let mut agent_for_sim = new_branch.accumulated_agent.clone();
                let world_for_sim = new_branch.accumulated_world.clone();

                for (chain_position, fact_name, object_ids) in &new_branch.action_bindings {
                    if *chain_position == insert_pos as i64 && !object_ids.is_empty() {
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

                let (after_agent, after_world) = super::call_apply_effect(
                    action.effect_callable_id,
                    agent_for_sim,
                    world_for_sim,
                    self.ctx.request_tx,
                );
                new_branch.accumulated_agent = after_agent;
                new_branch.accumulated_world = after_world;
            }

            let min_action_cost = self.ctx.min_action_cost;
            let min_provision_cost = self.ctx.min_provision_cost;
            let estimated_remaining = heuristic::estimate_remaining(
                &new_branch,
                min_action_cost,
                min_provision_cost,
            );

            successors.push(SearchNode {
                branch: new_branch,
                depth: node.depth + 1,
                estimated_remaining,
            });
        }

        successors
    }
}

pub fn find_candidate_actions(
    branch: &PlanBranch,
    ctx: &SearchContext,
) -> Vec<ActionCandidate> {
    let mut candidates: Vec<ActionCandidate> = vec![];

    for (idx, action) in ctx.actions.iter().enumerate() {
        if !super::action_is_valid(action, ctx) {
            ctx.tree_dump
                .borrow_mut()
                .exclude_action(&action.name, "dependencies invalid (object freed)");
            continue;
        }

        let action_candidates = action_candidates_for_needs(idx, action, branch, ctx);

        if !action_candidates.is_empty() {
            candidates.extend(action_candidates);
        } else {
            let reason =
                if branch.open_preconditions.is_empty() && branch.open_requirements.is_empty() {
                    "no open needs to satisfy".to_string()
                } else if !branch.open_requirements.is_empty() && action.provisions.is_empty() {
                    "no provisions to satisfy open requirements".to_string()
                } else if !branch.open_preconditions.is_empty() && action.effect_callable_id == 0 {
                    "no effect to satisfy open preconditions".to_string()
                } else {
                    format!(
                        "effect/provisions don't match open needs ({} pre, {} req)",
                        branch.open_preconditions.len(),
                        branch.open_requirements.len()
                    )
                };
            ctx.tree_dump
                .borrow_mut()
                .exclude_action(&action.name, &reason);
        }
    }

    candidates.sort_by(|a, b| {
        a.estimated_cost
            .partial_cmp(&b.estimated_cost)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.action_idx.cmp(&b.action_idx))
    });

    candidates
}

pub fn insertion_index_for_candidate(
    branch: &PlanBranch,
    candidate: &ActionCandidate,
) -> usize {
    if candidate.satisfied_requirement_indices.is_empty() {
        return 0;
    }

    candidate
        .satisfied_requirement_indices
        .iter()
        .filter_map(|idx| branch.open_requirement_consumers.get(*idx))
        .map(|consumer_position| *consumer_position as usize)
        .min()
        .map(|position| position.min(branch.action_chain.len()))
        .unwrap_or(0)
}

pub fn shift_branch_positions_for_insert(branch: &mut PlanBranch, insert_pos: usize) {
    let insert_pos = insert_pos as i64;
    for (chain_position, _, _) in &mut branch.action_bindings {
        if *chain_position >= insert_pos {
            *chain_position += 1;
        }
    }
    for consumer_position in &mut branch.open_requirement_consumers {
        if *consumer_position >= insert_pos {
            *consumer_position += 1;
        }
    }
}

fn action_candidates_for_needs(
    action_idx: usize,
    action: &ActionSpec,
    branch: &PlanBranch,
    ctx: &SearchContext,
) -> Vec<ActionCandidate> {
    let mut candidates = Vec::new();

    if !branch.open_requirements.is_empty() {
        for (req_idx, req) in branch.open_requirements.iter().enumerate() {
            for prov in &action.provisions {
                if super::provision_satisfies_requirement_in_context(prov, req, ctx.initial_world) {
                    let binding = extract_binding_for_requirement(prov, req);
                    let estimated_cost =
                        estimate_action_cost_with_binding(action, branch, ctx, &binding);
                    if estimated_cost != f64::INFINITY {
                        candidates.push(ActionCandidate {
                            action_idx,
                            estimated_cost,
                            satisfied_precondition_indices: vec![],
                            satisfied_requirement_indices: vec![req_idx],
                            requires_bound_effect: false,
                        });
                    }
                    break;
                }
            }
        }
    }

    if !branch.open_preconditions.is_empty() {
        let satisfied_indices = bound_effect_satisfied_precondition_indices(
            action,
            &branch.open_preconditions,
            &branch.bound_provisions,
            branch,
            ctx,
        );
        if !satisfied_indices.is_empty() {
            let estimated_cost = estimate_action_cost(action, branch, ctx);
            if estimated_cost != f64::INFINITY {
                candidates.push(ActionCandidate {
                    action_idx,
                    estimated_cost,
                    satisfied_precondition_indices: satisfied_indices,
                    satisfied_requirement_indices: vec![],
                    requires_bound_effect: false,
                });
            }
        } else if !action.requirements.is_empty() {
            let potential_satisfied_indices =
                potential_bound_effect_satisfied_precondition_indices(
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
                        satisfied_requirement_indices: vec![],
                        requires_bound_effect: true,
                    });
                }
            }
        }
    }

    candidates
}

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

    let (after_agent, after_world) = super::call_apply_effect(
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
        None => super::eval_precondition(precondition, agent, world, ctx.request_tx),
    }
}

fn create_hypothetical_snapshot(
    base_agent: &BlackboardSnapshot,
    base_world: &BlackboardSnapshot,
    requirements: &[RequirementSpec],
    bound_provisions: &[ProvisionSpec],
) -> Option<(BlackboardSnapshot, BlackboardSnapshot)> {
    if !super::requirements_satisfied_in_context(requirements, bound_provisions, base_world) {
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
            _ => {}
        }
    }

    Some((hypo_agent, hypo_world))
}

fn bound_value_for_requirement(
    requirement: &RequirementSpec,
    bound_provisions: &[ProvisionSpec],
    world: &BlackboardSnapshot,
) -> Option<VariantSnapshot> {
    bound_provisions
        .iter()
        .find_map(|provision| match provision {
            ProvisionSpec::Binding { value, .. }
                if super::provision_satisfies_requirement_in_context(
                    provision,
                    requirement,
                    world,
                ) =>
            {
                Some(value.clone())
            }
            _ => None,
        })
}

fn estimate_action_cost(
    action: &ActionSpec,
    branch: &PlanBranch,
    ctx: &SearchContext,
) -> f64 {
    estimate_action_cost_with_binding(action, branch, ctx, &None)
}

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

    super::call_get_cost(
        action.cost_callable_id,
        &agent_for_cost,
        &hypo_world,
        ctx.request_tx,
    )
}

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

pub fn update_open_needs(
    branch: &mut PlanBranch,
    candidate: &ActionCandidate,
    action: &ActionSpec,
    action_position: usize,
    ctx: &SearchContext,
) -> bool {
    let mut requirements_to_remove = Vec::new();
    let mut new_bindings = Vec::new();
    let mut newly_bound_provisions = Vec::new();
    for (req_idx, req) in branch.open_requirements.iter().enumerate() {
        if !candidate.satisfied_requirement_indices.is_empty()
            && !candidate.satisfied_requirement_indices.contains(&req_idx)
        {
            continue;
        }
        for prov in &action.provisions {
            if super::provision_satisfies_requirement_in_context(prov, req, ctx.initial_world) {
                requirements_to_remove.push(req_idx);
                match (prov, req) {
                    (
                        ProvisionSpec::FactWildcard {
                            fact_name: prov_name,
                        },
                        RequirementSpec::Fact {
                            fact_name: req_name,
                            args,
                        },
                    ) if prov_name == req_name => {
                        newly_bound_provisions.push(ProvisionSpec::Fact {
                            fact_name: prov_name.clone(),
                            args: args.clone(),
                        });
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
                            new_bindings.push((
                                action_position as i64,
                                prov_name.clone(),
                                object_ids,
                            ));
                        }
                    }
                    _ => newly_bound_provisions.push(prov.clone()),
                }
                break;
            }
        }
    }

    requirements_to_remove.sort();
    requirements_to_remove.reverse();
    for idx in requirements_to_remove {
        branch.open_requirements.remove(idx);
        branch.open_requirement_consumers.remove(idx);
    }

    for (chain_position, fact_name, object_ids) in new_bindings {
        branch
            .action_bindings
            .push((chain_position, fact_name, object_ids));
    }

    for prov in newly_bound_provisions {
        if !branch.bound_provisions.contains(&prov) {
            branch.bound_provisions.push(prov);
        }
    }

    for prov in &action.provisions {
        if matches!(prov, ProvisionSpec::FactWildcard { .. }) {
            continue;
        }
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

    for req in &action.requirements {
        if !branch.open_requirements.contains(req) {
            branch.open_requirements.push(req.clone());
            branch
                .open_requirement_consumers
                .push(action_position as i64);
        }
    }

    for precond in &action.preconditions {
        let already_satisfied = super::eval_precondition(
            precond,
            &branch.accumulated_agent,
            &branch.accumulated_world,
            ctx.request_tx,
        );

        if !already_satisfied {
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

pub struct SearchContext<'a> {
    pub actions: &'a [ActionSpec],
    pub goals: &'a [GoalSpec],
    pub initial_agent: &'a BlackboardSnapshot,
    pub initial_world: &'a BlackboardSnapshot,
    pub initial_provisions: &'a [ProvisionSpec],
    pub goal_preconditions: &'a [PreconditionSpec],
    pub max_depth: usize,
    pub request_tx: &'a Sender<CallbackRequest>,
    pub cancel_flag: &'a Arc<AtomicBool>,
    pub tree_dump: &'a RefCell<TreeDump>,
    pub min_action_cost: f64,
    pub min_provision_cost: f64,
}
