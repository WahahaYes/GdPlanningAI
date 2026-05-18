//! Shared simulation utilities for planning.

use crate::plan_types::*;
use crate::requirement::ProvisionSpec;
use crate::snapshot::{BlackboardSnapshot, VariantSnapshot};
use std::sync::mpsc::Sender;

/// Results of applying an action to a state.
pub struct ActionResult {
    pub agent: BlackboardSnapshot,
    pub world: BlackboardSnapshot,
    pub cost: f64,
    pub provisions: Vec<ProvisionSpec>,
}

/// Applies an action's bindings to a snapshot.
pub fn apply_bindings(
    agent: &mut BlackboardSnapshot,
    chain_position: usize,
    action_bindings: &[(i64, String, Vec<i64>)],
) {
    for (binding_chain_position, fact_name, object_ids) in action_bindings {
        if *binding_chain_position == chain_position as i64 && !object_ids.is_empty() {
            let id_variants: Vec<VariantSnapshot> = object_ids
                .iter()
                .map(|id| VariantSnapshot::ObjectRef(*id))
                .collect();
            let binding_value = VariantSnapshot::Array(id_variants);
            agent.properties.insert(fact_name.clone(), binding_value);
        }
    }
}

/// Checks if an action is valid in the current state.
pub fn is_action_valid(
    action: &ActionSpec,
    agent: &BlackboardSnapshot,
    world: &BlackboardSnapshot,
    provisions: &[ProvisionSpec],
    request_tx: &Sender<CallbackRequest>,
) -> bool {
    // 1. Dependent objects must exist
    if !super::check_dependencies_valid(&action.dependent_object_ids) {
        return false;
    }

    // 2. Validity checks must pass
    for check in &action.validity_checks {
        if !super::eval_precondition(check, agent, world, request_tx) {
            return false;
        }
    }

    // 3. Preconditions must pass
    for precond in &action.preconditions {
        if !super::eval_precondition(precond, agent, world, request_tx) {
            return false;
        }
    }

    // 4. Requirements must be satisfied
    if !super::requirements_satisfied_in_context(&action.requirements, provisions, world) {
        return false;
    }

    true
}

/// Simulates the application of an action to a state.
pub fn simulate_action(
    action: &ActionSpec,
    chain_position: usize,
    action_bindings: &[(i64, String, Vec<i64>)],
    agent: &BlackboardSnapshot,
    world: &BlackboardSnapshot,
    mut accumulated_provisions: Vec<ProvisionSpec>,
    request_tx: &Sender<CallbackRequest>,
    skip_validity: bool,
) -> Option<ActionResult> {
    let mut current_agent = agent.clone();
    let current_world = world.clone();

    // 1. Apply bindings
    apply_bindings(&mut current_agent, chain_position, action_bindings);

    // 2. Special case: if action provides FactWildcard, re-apply bindings after initial injection
    // (This matches the logic in forward_validate)
    if action
        .provisions
        .iter()
        .any(|p| matches!(p, ProvisionSpec::FactWildcard { .. }))
    {
        apply_bindings(&mut current_agent, chain_position, action_bindings);
    }

    // 3. Check validity
    if !skip_validity && !is_action_valid(action, &current_agent, &current_world, &accumulated_provisions, request_tx) {
        return None;
    }

    // 4. Get cost
    let cost = super::call_get_cost(action.cost_callable_id, &current_agent, &current_world, request_tx);
    if cost == f64::INFINITY {
        return None;
    }

    // 5. Apply effect
    let (mut new_agent, new_world) =
        super::call_apply_effect(action.effect_callable_id, current_agent, current_world, request_tx);
    
    // 6. Update provisions
    for prov in &action.provisions {
        match prov {
            ProvisionSpec::Binding { binding_name, value } => {
                // If it's a binding provision, we also update the agent property 
                // so subsequent actions in the forward chain can see it.
                if !new_agent.properties.contains_key(binding_name) {
                    new_agent.properties.insert(binding_name.clone(), value.clone());
                }
            }
            ProvisionSpec::FactWildcard { fact_name } => {
                let has_concrete_binding = action_bindings.iter().any(|(pos, _, ids)| {
                    *pos == chain_position as i64 && !ids.is_empty()
                });
                if !has_concrete_binding && !accumulated_provisions.contains(prov) {
                    accumulated_provisions.push(prov.clone());
                }
                continue;
            }
            _ => {}
        }

        if !accumulated_provisions.contains(prov) {
            accumulated_provisions.push(prov.clone());
        }
    }

    // Add concrete fact provisions from bindings
    for (pos, fact_name, object_ids) in action_bindings {
        if *pos == chain_position as i64 && !object_ids.is_empty() {
            let args: Vec<VariantSnapshot> = object_ids
                .iter()
                .map(|id| VariantSnapshot::ObjectRef(*id))
                .collect();
            let prov = ProvisionSpec::Fact {
                fact_name: fact_name.clone(),
                args,
            };
            if !accumulated_provisions.contains(&prov) {
                accumulated_provisions.push(prov);
            }
        }
    }

    Some(ActionResult {
        agent: new_agent,
        world: new_world,
        cost,
        provisions: accumulated_provisions,
    })
}
