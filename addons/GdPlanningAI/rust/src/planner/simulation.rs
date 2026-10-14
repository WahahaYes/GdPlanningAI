//! Simulation logic for evaluating action effects and preconditions.
//!
//! This module handles communication with Godot (via callbacks) to evaluate
//! dynamic properties that cannot be calculated purely in Rust.
//!
//! The request/resume pattern used by both entry points:
//! 1. The function is called with no response. If it needs GDScript (a cost callable, an
//!    effect callable or a custom precondition), it allocates a request id, sends a
//!    [`CallbackRequest`] on `ctx.request_tx` and returns [`StepResult::Pending`] with that id.
//! 2. The caller parks its search node under the id. When the main thread answers, the node is
//!    re-enqueued with the answer attached as `response`.
//! 3. The function is called again with the same arguments plus `response`, consumes it, and
//!    either completes or asks for the next callback (a cost request is followed by an effect
//!    request, for example).
//!
//! Functions here never block and never touch Godot objects, so they are safe to run on the
//! planner thread. Bindings travel with each request and are injected into the agent
//! blackboard on the main thread.

use crate::plan_types::*;
use crate::planner::types::SearchContext;
use crate::snapshot::{BlackboardSnapshot, VariantSnapshot};

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
    /// Terminal success for a verification pass: the branch was recorded as the best plan.
    /// Only the engine's verification steps produce this; [`eval_precondition`] and
    /// [`simulate_action`] never do.
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

/// Evaluates a precondition against the current state.
///
/// Builtin preconditions are evaluated immediately on the snapshots. A custom precondition
/// returns `StepResult::Pending` after sending an [`CallbackKind::EvalCustomPrecond`] request;
/// call again with the `Bool` response to get `Ready`. Never returns `Invalid` or `Complete`.
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

/// Simulates the cost and effect of an action against the given state.
///
/// This involves potentially two round-trips to Godot:
/// 1. Evaluate the action's cost.
/// 2. Simulate the action's effect on the agent and world snapshots.
///
/// An infinite cost returns `Invalid` before the effect is requested, so an action can
/// gate itself out of the search by returning infinity. Actions without a cost callable cost
/// `1.0`; actions without an effect callable leave the snapshots unchanged.
///
/// Used in two places: candidate discovery simulates against the *initial* state to learn
/// an action's cost and effect, and verification replays the chain against the *running*
/// simulated state.
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
