//! Resumable evaluation of action effects, costs and custom preconditions.
//!
//! Called without a response, a function that needs GDScript sends a [`CallbackRequest`] and
//! returns [`StepResult::Pending`] with its id; the caller parks the node and calls again with the
//! `response` once the main thread answers. Nothing here blocks or touches Godot objects.

use crate::plan_types::*;
use crate::planner::types::SearchContext;
use crate::snapshot::{BlackboardSnapshot, VariantSnapshot};
use std::sync::Arc;
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
    /// Agent state the action is simulated against (shared handle, never deep-cloned).
    pub agent: Arc<crate::snapshot::BlackboardSnapshot>,
    /// World state the action is simulated against (shared handle, never deep-cloned).
    pub world: Arc<crate::snapshot::BlackboardSnapshot>,
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
    agent: &Arc<crate::snapshot::BlackboardSnapshot>,
    world: &Arc<crate::snapshot::BlackboardSnapshot>,
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
                    agent: Arc::clone(agent),
                    world: Arc::clone(world),
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
                        agent: Arc::clone(&args.agent),
                        world: Arc::clone(&args.world),
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
            crate::effect_ir::apply_effect_spec(spec, &args.agent, &args.world, args.bindings);
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
                agent: new_agent.as_ref().clone(),
                world: new_world.as_ref().clone(),
                cost,
            })
        } else {
            // Fallback budget: custom GDScript effects stay rare and visible. One slot is
            // reserved atomically (try_update fails once the count reaches the budget),
            // so concurrent steps cannot over-admit; the reservation is released if the
            // channel is gone. At or over budget the requesting branch is Invalid (dropped);
            // the trip warns once per job and bumps the shared trips counter. Cost and
            // precondition callbacks are never counted here.
            //
            // Single-threaded-engine note: each engine runs one step at a time (it is moved
            // between worker threads but never shared), so in practice no two steps race
            // here; fetch_update keeps the count exact even if that ever changes. A closed
            // channel means the job is being torn down, so the branch is dropped (Invalid)
            // rather than parked on an answer that can never arrive.
            if args
                .ctx
                .effect_fallback_count
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |c| {
                    (c < args.ctx.effect_fallback_budget as u64).then_some(c + 1)
                })
                .is_err()
            {
                if !args
                    .ctx
                    .effect_fallback_warned
                    .swap(true, Ordering::Relaxed)
                {
                    log_warn!(
                        "Effect fallback budget exceeded (cap {}): dropping branch; custom effects should declare effect_spec",
                        args.ctx.effect_fallback_budget
                    );
                    args.ctx
                        .effect_fallback_trips
                        .fetch_add(1, Ordering::Relaxed);
                }
                return StepResult::Invalid;
            }
            let request_id = crate::plan_types::next_request_id();
            if args
                .ctx
                .request_tx
                .send(CallbackRequest {
                    request_id,
                    callable_id: id,
                    kind: CallbackKind::ApplyEffect {
                        agent: Arc::clone(&args.agent),
                        world: Arc::clone(&args.world),
                    },
                    bindings: args.bindings.to_vec(),
                    response_tx: args.ctx.engine_response_tx.clone(),
                })
                .is_err()
            {
                args.ctx
                    .effect_fallback_count
                    .fetch_sub(1, Ordering::Relaxed);
                return StepResult::Invalid;
            }
            StepResult::Pending(request_id)
        }
    } else {
        // No effect callable - identity effect
        StepResult::Ready(SimResult {
            agent: args.agent.as_ref().clone(),
            world: args.world.as_ref().clone(),
            cost,
        })
    }
}
