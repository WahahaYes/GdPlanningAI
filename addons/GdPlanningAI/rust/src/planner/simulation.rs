use crate::plan_types::*;
use crate::snapshot::{BlackboardSnapshot, VariantSnapshot};
use crate::requirement::ProvisionSpec;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_REQUEST_ID: AtomicUsize = AtomicUsize::new(1);

pub fn next_request_id() -> usize {
    NEXT_REQUEST_ID.fetch_add(1, Ordering::SeqCst)
}

#[derive(Debug, Clone)]
pub enum PreconditionResult {
    Ready(bool),
    Pending(usize),
}

#[derive(Debug, Clone)]
pub struct SimulationResult {
    pub agent: BlackboardSnapshot,
    pub world: BlackboardSnapshot,
    pub cost: f64,
}

#[derive(Debug, Clone)]
pub enum SimulationStepResult {
    Ready(SimulationResult),
    Pending(usize),
    Invalid,
}

use std::hash::{Hash, Hasher};

fn calculate_provisions_hash(provisions: &[ProvisionSpec]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    provisions.hash(&mut hasher);
    hasher.finish()
}

/// Evaluates a precondition against a state.
pub fn eval_precondition(
    precond: &PreconditionSpec,
    agent: &BlackboardSnapshot,
    world: &BlackboardSnapshot,
    provisions: Vec<ProvisionSpec>,
    bindings: Vec<(String, Vec<VariantSnapshot>)>,
    ctx: &super::expander::SearchContext,
) -> PreconditionResult {
    match precond {
        PreconditionSpec::Builtin { .. } => {
            PreconditionResult::Ready(precond.evaluate_builtin(agent, world).unwrap_or(false))
        }
        PreconditionSpec::Custom { callable_id, .. } => {
            let sim_key = SimulationKey {
                action_idx: None,
                kind: RequestKind::Precondition,
                agent_state_hash: agent.calculate_hash(),
                world_state_hash: world.calculate_hash(),
                provisions_hash: calculate_provisions_hash(&provisions),
            };

            // 1. Check if we already have a result
            {
                let results = ctx.callback_results.lock().unwrap();
                if let Some(CallbackResponse::Bool(b)) = results.get(&sim_key) {
                    log_debug!("Cache HIT: Precondition key {:?} -> {}", sim_key, b);
                    return PreconditionResult::Ready(*b);
                }
            }

            // 2. Check if a request is already pending
            {
                let pending = ctx.pending_requests.lock().unwrap();
                if let Some(request_id) = pending.get(&sim_key) {
                    log_debug!("Cache PENDING: Precondition key {:?} -> ID {}", sim_key, request_id);
                    return PreconditionResult::Pending(*request_id);
                }
            }
            
            log_debug!("Cache MISS: Precondition key {:?}", sim_key);

            // 3. Send new request
            let request_id = next_request_id();
            {
                let mut pending = ctx.pending_requests.lock().unwrap();
                pending.insert(sim_key, request_id);
            }

            let (tx, _) = std::sync::mpsc::channel();
            let request = CallbackRequest {
                request_id,
                sim_key,
                callable_id: *callable_id,
                kind: CallbackKind::EvalCustomPrecond {
                    agent: agent.clone(),
                    world: world.clone(),
                    provisions,
                    bindings,
                },
                response_tx: tx,
            };

            if ctx.request_tx.send(request).is_err() {
                return PreconditionResult::Ready(false);
            }

            PreconditionResult::Pending(request_id)
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
    ctx: &super::expander::SearchContext,
) -> SimulationStepResult {
    let action_idx = ctx.actions.iter().position(|a| a.name == action.name);
    
    // 1. Check Cost
    if let Some(_callable_id) = action.cost_callable_id {
        let cost_key = SimulationKey {
            action_idx,
            kind: RequestKind::Cost,
            agent_state_hash: agent.calculate_hash(),
            world_state_hash: world.calculate_hash(),
            provisions_hash: calculate_provisions_hash(&accumulated_provisions),
        };

        let cached_cost = {
            let results = ctx.callback_results.lock().unwrap();
            results.get(&cost_key).cloned()
        };

        if let Some(CallbackResponse::Float(f)) = cached_cost {
            log_debug!("Cache HIT: Cost key {:?} -> {}", cost_key, f);
            return simulate_effect_only(action, chain_position, action_bindings, agent, world, accumulated_provisions, ctx, f);
        }

        {
            let pending = ctx.pending_requests.lock().unwrap();
            if let Some(request_id) = pending.get(&cost_key) {
                log_debug!("Cache PENDING: Cost key {:?} -> ID {}", cost_key, request_id);
                return SimulationStepResult::Pending(*request_id);
            }
        }

        log_debug!("Cache MISS: Cost key {:?}", cost_key);

        // Send new Cost request
        let request_id = next_request_id();
        {
            let mut pending = ctx.pending_requests.lock().unwrap();
            pending.insert(cost_key, request_id);
        }

        let relevant_bindings: Vec<(String, Vec<VariantSnapshot>)> = action_bindings
            .iter()
            .filter(|(idx, _, _)| *idx == chain_position as i64)
            .map(|(_, name, ids)| (name.clone(), ids.clone()))
            .collect();

        let (tx, _) = std::sync::mpsc::channel();
        let request = CallbackRequest {
            request_id,
            sim_key: cost_key,
            callable_id: action.cost_callable_id.unwrap(),
            kind: CallbackKind::GetCost {
                agent: agent.clone(),
                world: world.clone(),
                provisions: accumulated_provisions.clone(),
                bindings: relevant_bindings,
            },
            response_tx: tx,
        };

        if ctx.request_tx.send(request).is_ok() {
            return SimulationStepResult::Pending(request_id);
        } else {
            return SimulationStepResult::Invalid;
        }
    }

    simulate_effect_only(action, chain_position, action_bindings, agent, world, accumulated_provisions, ctx, 1.0)
}

fn simulate_effect_only(
    action: &ActionSpec,
    chain_position: usize,
    action_bindings: &[(i64, String, Vec<VariantSnapshot>)],
    agent: &BlackboardSnapshot,
    world: &BlackboardSnapshot,
    accumulated_provisions: Vec<ProvisionSpec>,
    ctx: &super::expander::SearchContext,
    cost: f64,
) -> SimulationStepResult {
    if let Some(callable_id) = action.effect_callable_id {
        let action_idx = ctx.actions.iter().position(|a| a.name == action.name);
        let effect_key = SimulationKey {
            action_idx,
            kind: RequestKind::Effect,
            agent_state_hash: agent.calculate_hash(),
            world_state_hash: world.calculate_hash(),
            provisions_hash: calculate_provisions_hash(&accumulated_provisions),
        };

        let cached_effect = {
            let results = ctx.callback_results.lock().unwrap();
            results.get(&effect_key).cloned()
        };

        if let Some(CallbackResponse::UpdatedSnapshots(res_agent, res_world)) = cached_effect {
            log_debug!("Cache HIT: Effect key {:?}", effect_key);
            return SimulationStepResult::Ready(SimulationResult {
                agent: res_agent.clone(),
                world: res_world.clone(),
                cost,
            });
        }

        {
            let pending = ctx.pending_requests.lock().unwrap();
            if let Some(request_id) = pending.get(&effect_key) {
                log_debug!("Cache PENDING: Effect key {:?} -> ID {}", effect_key, request_id);
                return SimulationStepResult::Pending(*request_id);
            }
        }

        log_debug!("Cache MISS: Effect key {:?}", effect_key);

        let request_id = next_request_id();
        {
            let mut pending = ctx.pending_requests.lock().unwrap();
            pending.insert(effect_key, request_id);
        }

        let relevant_bindings: Vec<(String, Vec<VariantSnapshot>)> = action_bindings
            .iter()
            .filter(|(idx, _, _)| *idx == chain_position as i64)
            .map(|(_, name, ids)| (name.clone(), ids.clone()))
            .collect();

        let (tx, _) = std::sync::mpsc::channel();
        let request = CallbackRequest {
            request_id,
            sim_key: effect_key,
            callable_id,
            kind: CallbackKind::ApplyEffect {
                agent: agent.clone(),
                world: world.clone(),
                provisions: accumulated_provisions,
                bindings: relevant_bindings,
            },
            response_tx: tx,
        };

        if ctx.request_tx.send(request).is_ok() {
            return SimulationStepResult::Pending(request_id);
        } else {
            return SimulationStepResult::Invalid;
        }
    }

    SimulationStepResult::Ready(SimulationResult {
        agent: agent.clone(),
        world: world.clone(),
        cost,
    })
}
