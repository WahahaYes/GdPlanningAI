pub mod controller;
pub mod engine;
pub mod expander;
pub mod goal_selection;
pub mod heuristic;
pub mod policy;
pub mod stats;

use crate::debug_tree::TreeDump;
use crate::plan_tree::PlanResult;
use crate::plan_types::*;
use crate::precondition::PreconditionTarget;
use crate::requirement::{ProvisionSpec, RequirementSpec, extract_initial_provisions};
use crate::snapshot::{BlackboardSnapshot, VariantSnapshot};
use std::cell::RefCell;
use std::sync::mpsc::Sender;
use std::sync::{Arc, atomic::AtomicBool};

use controller::DfsController;
use engine::PlannerEngine;
use expander::SearchContext;
use goal_selection::HighestRewardFirst;
use policy::ExhaustivePolicy;

pub use engine::PlannerEngine as PlannerEngineType;
pub use expander::{PlanBranch, SearchContext as SearchContextType};

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

    let tree_dump = RefCell::new(TreeDump::new());

    let initial_provisions = extract_initial_provisions(&agent);

    let ctx = SearchContext {
        actions: &actions,
        goals: &goals,
        initial_agent: &agent,
        initial_world: &world,
        initial_provisions: &initial_provisions,
        goal_preconditions: &[],
        max_depth: max_recursion,
        request_tx: &request_tx,
        cancel_flag: &cancel_flag,
        tree_dump: &tree_dump,
        min_action_cost: 1.0,
        min_provision_cost: 1.0,
    };

    let controller = Box::new(DfsController::new());
    let policy = Box::new(ExhaustivePolicy::new());
    let goal_selection = Box::new(HighestRewardFirst::new(0));

    let mut engine = PlannerEngine::new(&ctx, controller, policy, goal_selection);

    let result = engine.run();

    if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
        let _ = result_tx.send(None);
        return;
    }

    match result {
        Some(plan) => {
            crate::log_debug!(
                "Found valid plan with cost {:.2}",
                plan.total_cost
            );
            let tree_output = tree_dump.borrow().format();
            if !tree_output.is_empty() {
                crate::log_debug!("{}", tree_output);
            }
            let _ = result_tx.send(Some(plan));
        }
        None => {
            let tree_output = tree_dump.borrow().format();
            if !tree_output.is_empty() {
                crate::log_debug!("{}", tree_output);
            }
            let _ = result_tx.send(Some(PlanResult::failure()));
        }
    }
}

pub(crate) fn forward_validate(
    action_chain: &[i64],
    action_bindings: &[(i64, String, Vec<i64>)],
    ctx: &SearchContext,
    goal_preconditions: &[PreconditionSpec],
) -> Option<(Vec<i64>, f64)> {
    let mut agent = ctx.initial_agent.clone();
    let mut world = ctx.initial_world.clone();
    let mut accumulated_provisions: Vec<ProvisionSpec> = ctx.initial_provisions.to_vec();
    let mut total_cost: f64 = 0.0;

    for (chain_position, action_idx) in action_chain.iter().enumerate() {
        let action = &ctx.actions[*action_idx as usize];

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

        if !check_dependencies_valid(&action.dependent_object_ids) {
            return None;
        }

        for check in &action.validity_checks {
            if !eval_precondition(check, &agent, &world, ctx.request_tx) {
                return None;
            }
        }

        for precond in &action.preconditions {
            if !eval_precondition(precond, &agent, &world, ctx.request_tx) {
                return None;
            }
        }

        if !requirements_satisfied_in_context(&action.requirements, &accumulated_provisions, &world)
        {
            return None;
        }

        if action
            .provisions
            .iter()
            .any(|p| matches!(p, ProvisionSpec::FactWildcard { .. }))
        {
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

        let cost = call_get_cost(action.cost_callable_id, &agent, &world, ctx.request_tx);
        if cost == f64::INFINITY {
            return None;
        }
        total_cost += cost;

        let (new_agent, new_world) =
            call_apply_effect(action.effect_callable_id, agent, world, ctx.request_tx);
        agent = new_agent;
        world = new_world;

        for prov in &action.provisions {
            if matches!(prov, ProvisionSpec::FactWildcard { .. }) {
                let has_concrete_binding =
                    action_bindings
                        .iter()
                        .any(|(binding_chain_position, _, object_ids)| {
                            *binding_chain_position == chain_position as i64
                                && !object_ids.is_empty()
                        });
                if !has_concrete_binding && !accumulated_provisions.contains(prov) {
                    accumulated_provisions.push(prov.clone());
                }
                continue;
            }
            if !accumulated_provisions.contains(prov) {
                accumulated_provisions.push(prov.clone());
            }
        }
        for (binding_chain_position, fact_name, object_ids) in action_bindings {
            if *binding_chain_position == chain_position as i64 && !object_ids.is_empty() {
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
    }

    if !goal_preconditions
        .iter()
        .all(|precond| eval_precondition(precond, &agent, &world, ctx.request_tx))
    {
        return None;
    }

    Some((action_chain.to_vec(), total_cost))
}

pub(crate) fn requirements_satisfied_in_context(
    requirements: &[RequirementSpec],
    provisions: &[ProvisionSpec],
    world: &BlackboardSnapshot,
) -> bool {
    requirements
        .iter()
        .all(|requirement| requirement_satisfied_in_context(requirement, provisions, world))
}

pub(crate) fn requirement_satisfied_in_context(
    requirement: &RequirementSpec,
    provisions: &[ProvisionSpec],
    world: &BlackboardSnapshot,
) -> bool {
    provisions
        .iter()
        .any(|provision| provision_satisfies_requirement_in_context(provision, requirement, world))
}

pub(crate) fn provision_satisfies_requirement_in_context(
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

pub(crate) fn binding_value_is_in_set(
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

pub(crate) fn action_is_valid(action: &ActionSpec, _ctx: &SearchContext) -> bool {
    check_dependencies_valid(&action.dependent_object_ids)
}

pub(crate) fn check_dependencies_valid(dependent_ids: &[i64]) -> bool {
    use godot::obj::InstanceId;
    use godot::prelude::Gd;

    dependent_ids.iter().all(|id| {
        let instance_id = InstanceId::from_i64(*id);
        Gd::<godot::prelude::Object>::try_from_instance_id(instance_id).is_ok()
    })
}

pub(crate) fn eval_precondition(
    spec: &PreconditionSpec,
    agent: &BlackboardSnapshot,
    world: &BlackboardSnapshot,
    request_tx: &Sender<CallbackRequest>,
) -> bool {
    match spec.evaluate_builtin(agent, world) {
        Some(result) => result,
        None => {
            if !check_dependencies_valid(spec.dependent_object_ids()) {
                return false;
            }

            let Some(callable_id) = spec.callable_id() else {
                return false;
            };
            call_eval_custom_precond(callable_id, agent, world, request_tx)
        }
    }
}

pub(crate) fn call_get_cost(
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

pub(crate) fn call_apply_effect(
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
        _ => (
            BlackboardSnapshot {
                properties: Default::default(),
                objects: Default::default(),
            },
            BlackboardSnapshot {
                properties: Default::default(),
                objects: Default::default(),
            },
        ),
    }
}

pub(crate) fn call_eval_custom_precond(
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
