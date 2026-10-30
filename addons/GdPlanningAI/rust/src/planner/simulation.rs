//! Resumable evaluation of action effects, costs and custom preconditions.
//!
//! Called without a response, a function that needs GDScript sends a [`CallbackRequest`] and
//! returns [`StepResult::Pending`] with its id; the caller parks the node and calls again with the
//! `response` once the main thread answers. Nothing here blocks or touches Godot objects.

use crate::plan_types::*;
use crate::planner::types::SearchContext;
use crate::snapshot::{BlackboardSnapshot, VariantSnapshot};
use std::sync::atomic::Ordering;

/// The result of a single simulation step.
#[derive(Debug, PartialEq)]
pub enum StepResult<T> {
    /// The step finished and produced a value.
    Ready(T),
    /// A callback request with this id was sent to the main thread; retry when it is answered.
    Pending(usize),
    /// The branch cannot be a valid plan (infinite cost, failed precondition or requirement)
    /// and should be dropped.
    Invalid,
    /// The branch was verified and recorded. Only produced by the engine's verification steps.
    Complete,
}

/// The result of a successful action simulation.
#[derive(Clone, Debug, PartialEq)]
pub struct SimResult {
    /// Agent state after the action's effect.
    pub agent: BlackboardSnapshot,
    /// World state after the action's effect.
    pub world: BlackboardSnapshot,
    /// The cost the action was charged.
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
    /// Agent state the action is simulated against.
    pub agent: &'a BlackboardSnapshot,
    /// World state the action is simulated against.
    pub world: &'a BlackboardSnapshot,
    /// Shared run context: action specs, request channel and discovery caches.
    pub ctx: &'a SearchContext,
    /// The main thread's answer to the last request this caller got `Pending` for, if the
    /// caller is resuming.
    pub response: Option<&'a CallbackResponse>,
    /// Per-branch cost cache. `simulate_action` reads the cached cost at
    /// `simulation_index` and writes the computed cost back to the same index.
    pub branch_action_costs: &'a mut [f64],
    /// Chain position being simulated; indexes `branch_action_costs`.
    pub simulation_index: usize,
    /// Bindings for this action occurrence, forwarded to every callback so GDScript sees them
    /// on the agent blackboard.
    pub bindings: &'a [(String, Vec<VariantSnapshot>)],
}

/// Evaluates a precondition on the snapshots. Builtins return `Ready` at once; custom ones
/// return `Pending` until called again with the `Bool` response.
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
                },
                bindings: bindings.to_vec(),
                response_tx: ctx.engine_response_tx.clone(),
            });
            StepResult::Pending(request_id)
        }
    }
}

/// Simulates an action's cost, then its effect, each possibly via a GDScript callback.
/// Infinite cost returns `Invalid` before the effect runs. The cost is cached in
/// `args.branch_action_costs[args.simulation_index]`, so pass the same slice when resuming.
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
                    },
                    bindings: args.bindings.to_vec(),
                    response_tx: args.ctx.engine_response_tx.clone(),
                });
                return StepResult::Pending(request_id);
            }
        }
    } else {
        1.0 // Default cost
    };

    if cost.is_infinite() {
        return StepResult::Invalid;
    }

    // Ensure the cost is recorded even for actions without a cost callable
    if args.simulation_index < args.branch_action_costs.len() {
        args.branch_action_costs[args.simulation_index] = cost;
    }

    // 2. Simulate Effect
    if let Some(spec) = &action.effect_spec {
        // Native spec: apply synchronously on the snapshots. Never pends, never callbacks.
        // The timer only runs at debug level, mirroring the transport timers.
        let timed = crate::logger::get_log_level() >= crate::logger::LogLevel::Debug;
        let start = timed.then(std::time::Instant::now);
        let (agent, world) =
            crate::effect_ir::apply_effect_spec(spec, args.agent, args.world, args.bindings);
        args.ctx.effect_ir_hits.fetch_add(1, Ordering::Relaxed);
        if let Some(start) = start {
            args.ctx
                .effect_ir_timings
                .lock()
                .unwrap()
                .push(start.elapsed().as_micros() as u64);
        }
        StepResult::Ready(SimResult { agent, world, cost })
    } else if let Some(id) = action.effect_callable_id {
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
                },
                bindings: args.bindings.to_vec(),
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
