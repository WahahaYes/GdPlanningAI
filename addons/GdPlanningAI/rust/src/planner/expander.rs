//! Plan branch expansion and candidate search.
//!
//! Handles the discovery of actions that can satisfy open preconditions or requirements,
//! and manages the resulting search tree successors.

use crate::debug_tree::TreeDump;
use crate::plan_types::*;
use crate::requirement::{ProvisionSpec, RequirementSpec};
use crate::snapshot::{BlackboardSnapshot, VariantSnapshot};
use std::cell::RefCell;
use std::sync::mpsc::Sender;
use std::sync::{Arc, atomic::AtomicBool};

use super::controller::SearchNode;
use super::heuristic;

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
}

/// Represents a partially constructed plan with open needs.
#[derive(Clone)]
pub struct PlanBranch {
    pub goal_preconditions: Vec<PreconditionSpec>,
    pub open_preconditions: Vec<PreconditionSpec>,
    pub open_requirements: Vec<RequirementSpec>,
    pub action_chain: Vec<i64>,
    pub bound_provisions: Vec<ProvisionSpec>,
    pub action_bindings: Vec<(i64, String, Vec<i64>)>,
    pub estimated_cost: f64,
    /// Snapshots at each step. 
    /// snapshots[0] is the state BEFORE action_chain[0] (initial state).
    /// snapshots[i+1] is the state AFTER action_chain[i].
    pub agent_snapshots: Vec<BlackboardSnapshot>,
    pub world_snapshots: Vec<BlackboardSnapshot>,
}

impl PlanBranch {
    pub fn initial_agent(&self) -> &BlackboardSnapshot {
        &self.agent_snapshots[0]
    }

    pub fn initial_world(&self) -> &BlackboardSnapshot {
        &self.world_snapshots[0]
    }

    pub fn accumulated_agent(&self) -> &BlackboardSnapshot {
        self.agent_snapshots.last().unwrap()
    }

    pub fn accumulated_world(&self) -> &BlackboardSnapshot {
        self.world_snapshots.last().unwrap()
    }

    /// Create a new root plan branch.
    pub fn new(
        goal_preconditions: &[PreconditionSpec],
        initial_provisions: &[ProvisionSpec],
        initial_agent: &BlackboardSnapshot,
        initial_world: &BlackboardSnapshot,
    ) -> Self {
        Self {
            goal_preconditions: goal_preconditions.to_vec(),
            open_preconditions: goal_preconditions.to_vec(),
            open_requirements: vec![],
            action_chain: vec![],
            bound_provisions: initial_provisions.to_vec(),
            action_bindings: vec![],
            estimated_cost: 0.0,
            agent_snapshots: vec![initial_agent.clone()],
            world_snapshots: vec![initial_world.clone()],
        }
    }

    /// Returns true if all preconditions and requirements are satisfied.
    pub fn is_complete(&self, request_tx: &Sender<CallbackRequest>) -> bool {
        // 1. Symbolic needs must be cleared
        if !self.open_preconditions.is_empty() || !self.open_requirements.is_empty() {
            return false;
        }

        // 2. Goal must be truly satisfied by the deep simulation (last snapshot)
        self.goal_preconditions.iter().all(|p| {
            super::eval_precondition(
                p,
                self.accumulated_agent(),
                self.accumulated_world(),
                request_tx,
            )
        })
    }
}

/// Handles expansion of search nodes into successors.
pub struct BranchExpander<'ctx> {
    pub ctx: &'ctx SearchContext<'ctx>,
}

impl<'ctx> BranchExpander<'ctx> {
    /// Expand a search node into candidate successor nodes.
    pub fn expand(&self, node: &SearchNode) -> Vec<SearchNode> {
        if node.branch.is_complete(self.ctx.request_tx) {
            return vec![];
        }

        let candidates = find_candidate_actions(&node.branch, node.tree_node_id, self.ctx);
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

            // 1. Position shifting and prepending
            let mut new_branch = node.branch.clone();
            shift_branch_positions_for_insert(&mut new_branch, 0);
            new_branch.action_chain.insert(0, candidate.action_idx as i64);
            new_branch.estimated_cost += candidate.estimated_cost;

            // 2. Update open needs (Adds bindings and new requirements)
            if !update_open_needs(&mut new_branch, candidate, action, self.ctx) {
                continue;
            }

            // 3. Perform the simulation of the NEW action at index 0
            // Initial state is snapshots[0].
            let sim_before_agent = &new_branch.agent_snapshots[0];
            let sim_before_world = &new_branch.world_snapshots[0];
            
            let mut current_provisions = self.ctx.initial_provisions.to_vec();

            let sim_result = super::simulation::simulate_action(
                action,
                0, // Prepend position
                &new_branch.action_bindings,
                sim_before_agent,
                sim_before_world,
                current_provisions,
                self.ctx.request_tx,
                true, // skip_validity = true (optimistic)
            );

            if let Some(result) = sim_result {
                // Insert the new state at index 1
                new_branch.agent_snapshots.insert(1, result.agent);
                new_branch.world_snapshots.insert(1, result.world);
                // Start with the ground-truth cost of the new first action
                new_branch.estimated_cost = result.cost;
            } else {
                continue;
            }

            // 4. Ripple simulation forward through the rest of the chain
            let mut ripple_failed = false;
            for pos in 1..new_branch.action_chain.len() {
                let action_idx = new_branch.action_chain[pos];
                let chain_action = &self.ctx.actions[action_idx as usize];
                
                let prev_agent = &new_branch.agent_snapshots[pos];
                let prev_world = &new_branch.world_snapshots[pos];
                
                let suffix_sim = super::simulation::simulate_action(
                    chain_action,
                    pos,
                    &new_branch.action_bindings,
                    prev_agent,
                    prev_world,
                    new_branch.bound_provisions.clone(),
                    self.ctx.request_tx,
                    true,
                );

                if let Some(res) = suffix_sim {
                    new_branch.agent_snapshots[pos + 1] = res.agent;
                    new_branch.world_snapshots[pos + 1] = res.world;
                    new_branch.estimated_cost += res.cost;
                } else {
                    ripple_failed = true;
                    break;
                }
            }

            if ripple_failed {
                continue;
            }

            // 5. Finalize estimated remaining cost
            let min_action_cost = self.ctx.min_action_cost;
            let min_provision_cost = self.ctx.min_provision_cost;
            let estimated_remaining =
                heuristic::estimate_remaining(&new_branch, min_action_cost, min_provision_cost);

            let open_precond_names: Vec<String> = new_branch
                .open_preconditions
                .iter()
                .map(|p| format!("{:?}", p))
                .collect();
            let open_req_names: Vec<String> = new_branch
                .open_requirements
                .iter()
                .map(|r| format!("{:?}", r))
                .collect();
            let _: usize = 0; // type hint for compiler
            let child_id = self.ctx.tree_dump.borrow_mut().add_child(
                node.tree_node_id,
                &action.name,
                candidate.estimated_cost,
                new_branch.estimated_cost,
                &open_precond_names,
                &open_req_names,
            );
            successors.push(SearchNode {
                branch: new_branch,
                depth: node.depth + 1,
                estimated_remaining,
                tree_node_id: child_id,
            });
        }

        successors
    }
}

/// Find all candidate actions that can satisfy the open needs of a branch.
pub fn find_candidate_actions(
    branch: &PlanBranch,
    tree_node_id: usize,
    ctx: &SearchContext,
) -> Vec<ActionCandidate> {
    let mut candidates: Vec<ActionCandidate> = vec![];
    for (idx, action) in ctx.actions.iter().enumerate() {
        /*
        if !super::action_is_valid(action, ctx) {
            ctx.tree_dump.borrow_mut().exclude_action(
                tree_node_id,
                &action.name,
                "dependencies invalid (object freed)",
            );
            continue;
        }
        */

        let action_candidates = action_candidates_for_needs(idx, action, branch, ctx);

        if !action_candidates.is_empty() {
            candidates.extend(action_candidates);
        } else {
            let reason =
                if branch.open_preconditions.is_empty() && branch.open_requirements.is_empty() {
                    "no open needs to satisfy".to_string()
                } else if !branch.open_requirements.is_empty() && action.provisions.is_empty() {
                    "no provisions to satisfy open requirements".to_string()
                } else if !branch.open_preconditions.is_empty() && action.effect_callable_id.is_none() {
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
                .exclude_action(tree_node_id, &action.name, &reason);
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

/// Shift stored positions in the branch to accommodate a new insertion at the start.
pub fn shift_branch_positions_for_insert(branch: &mut PlanBranch, _insert_pos: usize) {
    for (chain_position, _, _) in &mut branch.action_bindings {
        *chain_position += 1;
    }
}

fn action_candidates_for_needs(
    action_idx: usize,
    action: &ActionSpec,
    branch: &PlanBranch,
    ctx: &SearchContext,
) -> Vec<ActionCandidate> {
    let mut candidates = Vec::new();

    // Initial state is always snapshots[0]
    let initial_agent = &branch.agent_snapshots[0];
    let initial_world = &branch.world_snapshots[0];

    // 1. Check Requirements
    if !branch.open_requirements.is_empty() {
        for (req_idx, req) in branch.open_requirements.iter().enumerate() {
            for prov in &action.provisions {
                if super::provision_satisfies_requirement_in_context(prov, req, initial_world) {
                    let binding = extract_binding_for_requirement(prov, req);
                    let estimated_cost =
                        estimate_action_cost_with_binding(action, branch, ctx, &binding);
                    if estimated_cost != f64::INFINITY {
                        candidates.push(ActionCandidate {
                            action_idx,
                            estimated_cost,
                            satisfied_precondition_indices: vec![],
                            satisfied_requirement_indices: vec![req_idx],
                        });
                    }
                    break;
                }
            }
        }
    }

    // 2. Check Preconditions (Optimistically)
    if !branch.open_preconditions.is_empty() {
        // We use strict=false (potential) because in backward search, we must consider
        // an action even if its requirements aren't satisfied yet.
        let satisfied_indices = potential_bound_effect_satisfied_precondition_indices(
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
                });
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
    let initial_agent = &branch.agent_snapshots[0];
    let initial_world = &branch.world_snapshots[0];

    let Some((hypo_agent, hypo_world)) = create_hypothetical_snapshot(
        initial_agent,
        initial_world,
        &action.requirements,
        bound_provisions,
        true, // strict = true
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
    let initial_agent = &branch.agent_snapshots[0];
    let initial_world = &branch.world_snapshots[0];

    // Truly optimistic: ignore requirement satisfaction and just inject placeholders
    let Some((hypo_agent, hypo_world)) = create_hypothetical_snapshot(
        initial_agent,
        initial_world,
        &action.requirements,
        bound_provisions,
        false, // strict = false
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
    strict: bool,
) -> Option<(BlackboardSnapshot, BlackboardSnapshot)> {
    if strict && !super::requirements_satisfied_in_context(requirements, bound_provisions, base_world) {
        return None;
    }

    let mut hypo_agent = base_agent.clone();
    let hypo_world = base_world.clone();

    for req in requirements {
        match req {
            RequirementSpec::BindingExists { binding_name } => {
                if let Some(value) = bound_value_for_requirement(req, bound_provisions, base_world) {
                    hypo_agent.properties.insert(binding_name.clone(), value);
                } else if strict {
                    return None;
                } else {
                    // Optimistic placeholder
                    hypo_agent.properties.insert(binding_name.clone(), VariantSnapshot::Str("optimistic_placeholder".to_string()));
                }
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
                if let Some(value) = bound_value_for_requirement(req, bound_provisions, base_world) {
                    hypo_agent.properties.insert(binding_name.clone(), value);
                } else if strict {
                    return None;
                } else {
                    // Optimistic placeholder
                    hypo_agent.properties.insert(binding_name.clone(), VariantSnapshot::Str("optimistic_placeholder".to_string()));
                }
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

fn estimate_action_cost(action: &ActionSpec, branch: &PlanBranch, ctx: &SearchContext) -> f64 {
    estimate_action_cost_with_binding(action, branch, ctx, &None)
}

fn estimate_action_cost_with_binding(
    action: &ActionSpec,
    branch: &PlanBranch,
    ctx: &SearchContext,
    binding: &Option<(String, Vec<i64>)>,
) -> f64 {
    let initial_agent = &branch.agent_snapshots[0];
    let initial_world = &branch.world_snapshots[0];

    let Some((hypo_agent, hypo_world)) = create_hypothetical_snapshot(
        initial_agent,
        initial_world,
        &action.requirements,
        &branch.bound_provisions,
        false, // strict = false
    ) else {
        return super::call_get_cost(
            action.cost_callable_id,
            initial_agent,
            initial_world,
            ctx.request_tx,
        );
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

/// Update open needs and requirements after prepending an action.
pub fn update_open_needs(
    branch: &mut PlanBranch,
    candidate: &ActionCandidate,
    action: &ActionSpec,
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
                                0, // Prepend position
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

    // 2. Remove satisfied open needs
    for idx in candidate.satisfied_requirement_indices.iter().rev() {
        if *idx < branch.open_requirements.len() {
            branch.open_requirements.remove(*idx);
        }
    }

    remove_preconditions_by_index(
        &mut branch.open_preconditions,
        &candidate.satisfied_precondition_indices,
    );

    // 3. Add new needs from action
    for req in &action.requirements {
        if !branch.open_requirements.contains(req) {
            branch.open_requirements.push(req.clone());
        }
    }

    for precond in &action.preconditions {
        // Evaluate new action's preconditions against the state BEFORE it (snapshots[0])
        let already_satisfied = super::eval_precondition(
            precond,
            &branch.agent_snapshots[0],
            &branch.world_snapshots[0],
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

    true
}

fn remove_preconditions_by_index(
    preconditions: &mut Vec<PreconditionSpec>,
    indices: &[usize],
) {
    let mut sorted_indices = indices.to_vec();
    sorted_indices.sort_unstable();
    sorted_indices.dedup();

    for idx in sorted_indices.into_iter().rev() {
        if idx < preconditions.len() {
            preconditions.remove(idx);
        }
    }
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

/// Shared context for the search process.
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
    pub ripple_policy: RipplePolicy,
}
