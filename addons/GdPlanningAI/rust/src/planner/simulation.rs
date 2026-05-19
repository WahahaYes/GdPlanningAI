use crate::plan_types::*;
use crate::snapshot::{BlackboardSnapshot, VariantSnapshot};
use crate::requirement::ProvisionSpec;
use std::sync::mpsc::Sender;

pub struct SimulationResult {
    pub agent: BlackboardSnapshot,
    pub world: BlackboardSnapshot,
    pub cost: f64,
}

/// Evaluates a precondition against a state.
pub fn eval_precondition(
    precond: &PreconditionSpec,
    agent: &BlackboardSnapshot,
    world: &BlackboardSnapshot,
    provisions: Vec<ProvisionSpec>,
    bindings: Vec<(String, Vec<VariantSnapshot>)>,
    request_tx: &Sender<CallbackRequest>,
) -> bool {
    match precond {
        PreconditionSpec::Builtin { .. } => {
            precond.evaluate_builtin(agent, world).unwrap_or(false)
        }
        PreconditionSpec::Custom { callable_id, .. } => {
            // Request GDScript evaluation
            let (tx, rx) = std::sync::mpsc::channel();
            let request = CallbackRequest {
                callable_id: *callable_id,
                kind: CallbackKind::EvalCustomPrecond {
                    agent: agent.clone(),
                    world: world.clone(),
                    provisions,
                    bindings,
                },
                response_tx: tx,
            };

            if request_tx.send(request).is_err() {
                return false;
            }

            match rx.recv() {
                Ok(CallbackResponse::Bool(b)) => b,
                _ => false,
            }
        }
    }
}

/// Simulates an action's effect and cost.
pub fn simulate_action(
    action: &ActionSpec,
    chain_position: usize,
    action_bindings: &[(i64, String, Vec<VariantSnapshot>)],
    agent: &BlackboardSnapshot,
    world: &BlackboardSnapshot,
    accumulated_provisions: Vec<ProvisionSpec>,
    request_tx: &Sender<CallbackRequest>,
) -> Option<SimulationResult> {
    // Resolve bindings for this specific action in the chain
    let relevant_bindings: Vec<(String, Vec<VariantSnapshot>)> = action_bindings
        .iter()
        .filter(|(idx, _, _)| *idx == chain_position as i64)
        .map(|(_, name, ids)| (name.clone(), ids.clone()))
        .collect();

    // Call GDScript for cost and effect
    let mut cost = 1.0;
    if let Some(callable_id) = action.cost_callable_id {
        let (tx, rx) = std::sync::mpsc::channel();
        let request = CallbackRequest {
            callable_id,
            kind: CallbackKind::GetCost {
                agent: agent.clone(),
                world: world.clone(),
                provisions: accumulated_provisions.clone(),
                bindings: relevant_bindings.clone(),
            },
            response_tx: tx,
        };

        if request_tx.send(request).is_ok() {
            if let Ok(CallbackResponse::Float(f)) = rx.recv() {
                cost = f;
            }
        }
    }

    let mut new_agent = agent.clone();
    let mut new_world = world.clone();

    if let Some(callable_id) = action.effect_callable_id {
        let (tx, rx) = std::sync::mpsc::channel();
        let request = CallbackRequest {
            callable_id,
            kind: CallbackKind::ApplyEffect {
                agent: agent.clone(),
                world: world.clone(),
                provisions: accumulated_provisions,
                bindings: relevant_bindings,
            },
            response_tx: tx,
        };

        if request_tx.send(request).is_ok() {
            if let Ok(CallbackResponse::UpdatedSnapshots(res_agent, res_world)) = rx.recv() {
                new_agent = res_agent;
                new_world = res_world;
            } else {
                return None; // Effect call failed
            }
        } else {
            return None; // Channel error
        }
    }

    Some(SimulationResult {
        agent: new_agent,
        world: new_world,
        cost,
    })
}
