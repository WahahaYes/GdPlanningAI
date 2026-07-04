//! Simulation logic for evaluating action effects and preconditions.
//!
//! This module handles communication with Godot (via callbacks) to evaluate
//! dynamic properties that cannot be calculated purely in Rust.

use crate::plan_types::*;
use crate::planner::types::SearchContext;
use crate::snapshot::{BlackboardSnapshot, VariantSnapshot};

/// The result of a single simulation step.
pub enum StepResult<T> {
    Ready(T),
    Pending(usize),
    Invalid,
    Complete, // Terminal success for a simulation pass
}

/// The result of a successful action simulation.
pub struct SimResult {
    pub agent: BlackboardSnapshot,
    pub world: BlackboardSnapshot,
    pub cost: f64,
}

/// Parameters for action simulation.
///
/// The `branch_action_costs` slice serves as a per-branch action cost cache.
/// `simulate_action` both reads from and writes back to
/// `branch_action_costs[simulation_index]`, so callers must pass the same
/// mutable slice across resumptions of a single branch to avoid redundant
/// Godot callback round-trips.
pub struct SimArgs<'a> {
    pub agent: &'a BlackboardSnapshot,
    pub world: &'a BlackboardSnapshot,
    pub ctx: &'a SearchContext,
    pub response: Option<&'a CallbackResponse>,
    /// Per-branch cost cache. `simulate_action` reads the cached cost at
    /// `simulation_index` and writes the computed cost back to the same index.
    pub branch_action_costs: &'a mut [f64],
    pub simulation_index: usize,
    pub bindings: &'a [(String, Vec<VariantSnapshot>)],
}

/// Evaluates a precondition against the current state.
///
/// If the precondition is custom, this may return `StepResult::Pending` and
/// require a Godot callback.
pub fn eval_precondition(
    spec: &PreconditionSpec,
    agent: &BlackboardSnapshot,
    world: &BlackboardSnapshot,
    ctx: &SearchContext,
    response: Option<&CallbackResponse>,
    bindings: &[(String, Vec<VariantSnapshot>)],
) -> StepResult<bool> {
    match spec {
        PreconditionSpec::Builtin { .. } => {
            StepResult::Ready(spec.evaluate_builtin(agent, world).unwrap_or(false))
        }
        PreconditionSpec::Custom { callable_id, .. } => {
            if let Some(CallbackResponse::Bool(b)) = response {
                return StepResult::Ready(*b);
            }
            let request_id = crate::plan_types::next_request_id();
            let _ = ctx.request_tx.send(CallbackRequest {
                request_id,
                callable_id: *callable_id,
                kind: CallbackKind::EvalCustomPrecond {
                    agent: agent.clone(),
                    world: world.clone(),
                    provisions: vec![], // Preconditions don't use bound provisions yet
                    bindings: bindings.to_vec(),
                },
                response_tx: ctx.engine_response_tx.clone(),
            });
            StepResult::Pending(request_id)
        }
    }
}

/// Simulates the cost and effect of an action against the current state.
///
/// This involves potentially two round-trips to Godot:
/// 1. Evaluate the action's cost.
/// 2. Simulate the action's effect on the agent and world snapshots.
///
/// # Cost caching
///
/// The function reads and writes `args.branch_action_costs[args.simulation_index]`
/// as a cache. If a non-negative cost is already present, it is returned
/// directly. Otherwise the cost is computed (from the callback response or
/// the global discovery cache) and written back to the same index. Callers
/// must provide the same mutable slice on resumption so that previously
/// fetched costs are reused.
pub fn simulate_action(action_idx: usize, args: SimArgs) -> StepResult<SimResult> {
    let action = &args.ctx.actions[action_idx];

    // 1. Evaluate Cost
    let cost = if let Some(id) = action.cost_callable_id {
        if args.simulation_index < args.branch_action_costs.len()
            && args.branch_action_costs[args.simulation_index] >= 0.0
        {
            args.branch_action_costs[args.simulation_index]
        } else if let Some(CallbackResponse::Float(f)) = args.response {
            if args.simulation_index < args.branch_action_costs.len() {
                args.branch_action_costs[args.simulation_index] = *f;
            }
            *f
        } else {
            // Check global discovery cost cache before firing a new callback
            let costs = args.ctx.discovery_costs.lock().unwrap();
            let key = (action_idx, args.bindings.to_vec());
            if let Some(&cached_cost) = costs.get(&key) {
                drop(costs);
                if args.simulation_index < args.branch_action_costs.len() {
                    args.branch_action_costs[args.simulation_index] = cached_cost;
                }
                cached_cost
            } else {
                drop(costs);
                let request_id = crate::plan_types::next_request_id();
                let _ = args.ctx.request_tx.send(CallbackRequest {
                    request_id,
                    callable_id: id,
                    kind: CallbackKind::GetCost {
                        agent: args.agent.clone(),
                        world: args.world.clone(),
                        provisions: action.provisions.clone(),
                        bindings: args.bindings.to_vec(),
                    },
                    response_tx: args.ctx.engine_response_tx.clone(),
                });
                return StepResult::Pending(request_id);
            }
        }
    } else {
        1.0 // Default cost
    };

    if cost == f64::INFINITY {
        return StepResult::Invalid;
    }

    // Ensure the cost is recorded even for actions without a cost callable
    if args.simulation_index < args.branch_action_costs.len() {
        args.branch_action_costs[args.simulation_index] = cost;
    }

    // 2. Simulate Effect
    if let Some(id) = action.effect_callable_id {
        if let Some(CallbackResponse::UpdatedSnapshots(new_agent, new_world)) = args.response {
            StepResult::Ready(SimResult {
                agent: new_agent.clone(),
                world: new_world.clone(),
                cost,
            })
        } else {
            let request_id = crate::plan_types::next_request_id();
            let _ = args.ctx.request_tx.send(CallbackRequest {
                request_id,
                callable_id: id,
                kind: CallbackKind::ApplyEffect {
                    agent: args.agent.clone(),
                    world: args.world.clone(),
                    provisions: action.provisions.clone(),
                    bindings: args.bindings.to_vec(),
                },
                response_tx: args.ctx.engine_response_tx.clone(),
            });
            StepResult::Pending(request_id)
        }
    } else {
        // No effect callable - identity effect
        StepResult::Ready(SimResult {
            agent: args.agent.clone(),
            world: args.world.clone(),
            cost,
        })
    }
}
