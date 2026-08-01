# Research: Rust Planning Engine

**Date**: 2026-08-01 **Source**: `addons/GdPlanningAI/rust/` — audited via explore agent (bg_11c61a52) **Status**: Research dump — source-level truth for docs/ALGORITHM.md

______________________________________________________________________

## Crate overview

- Crate `gdplanningai-rust` v0.1.0, edition 2024, `crate-type = ["cdylib", "lib"]` (loads as GDExtension into Godot 4, links for tests).
- Deps: `godot = "0.4.5"` (gdext), `serde` + `serde_json` (declared but unused in `src/` — dead deps), `rayon = "1.8"` (thread pool). Build-dep `godot-bindings = "0.4.5"`. `build.rs` is a near-empty stub required by gdext.
- GDExtension entry: `GdPlanningAIExt` (`lib.rs:31-39`), `on_level_init` at `InitLevel::Servers` calls `logger::init_log_channel()`.
- Godot-exposed classes: `GdPAIBlackboard`, `SimObjectProxy`, `GdPAIPlanScheduler`.

## Module map

| Module | Role | |--------|------| | `lib.rs` | Crate root, module declarations, GDExtension entry | | `planner/mod.rs` | `SearchHeuristic` trait, Dijkstra/A\* heuristics, `TerminationStrategy` | | `planner/engine.rs` | `PlannerEngine` — search state machine (`step_search`, `process_simulation`) | | `planner/expander.rs` | Candidate discovery (`find_candidates`, `get_discovery_result`) | | `planner/simulation.rs` | `StepResult`/`SimResult`, `eval_precondition`, `simulate_action` | | `planner/types.rs` | `PlanBranch`, `SearchNode`, `PriorityNode`, `SearchContext`, insertion logic | | `plan_types.rs` | `ActionSpec`, `GoalSpec`, `PreconditionSpec`, callback channel messages | | `requirement.rs` | `RequirementSpec`/`ProvisionSpec` enums, matching/holding predicates | | `scheduler.rs` | `GdPAIPlanScheduler` — job lifecycle, callable registry, dispatch | | `snapshot.rs` | `VariantSnapshot`, `SimObjectData`, `BlackboardSnapshot` (send-safe mirrors) | | `gdpai_blackboard.rs` | `GdPAIBlackboard` Godot class (agent/world state) | | `precondition.rs` | `PreconditionHandler` (main-thread reconstruction for pre-filter) | | `plan_tree.rs` | `PlanResult` returned to GDScript | | `sim_object_proxy.rs` | `SimObjectProxy` — world object snapshot | | `debug_tree.rs` | `TreeDump` search-tree visualization structures | | `logger.rs` | Tiered logging macros + thread-safe log channel |

Stale doc references: `plan_types.rs:236,251` still mention `crate::action::ActionData` / `crate::goal::GoalData` — those modules no longer exist; the types now arrive as GDScript `VarDictionary`s.

## Overall flow (scheduler → engine → expander → simulation)

1. GDScript calls `GdPAIPlanScheduler.submit_plan(agent, agent_bb, world_bb, actions, goals, max_recursion, iteration_budget)` (`scheduler.rs:238`).
1. Blackboards → `BlackboardSnapshot`; action/goal dicts → `ActionSpec`/`GoalSpec` (callables registered into a per-job registry; specs carry **ids** only).
1. Satisfied-goal pre-filter: highest-reward goal already satisfied by the initial state is remembered (`satisfied_goal_index`); satisfied goals dropped; remainder sorted reward-desc (`scheduler.rs:276-299`).
1. Provision index built: `(ProvisionKind, name) → action indices`; `non_wildcard_actions` collected (`scheduler.rs:325-349`).
1. `SearchContext` + `PlannerEngine` built (Dijkstra, BestCost, budget; channels `req_tx`/`engine_response_tx`) (`scheduler.rs:351-371`).
1. Rayon spawn → worker runs `engine.plan(&goals)`, sends back `(PlannerRunResult, PlannerEngine)` (engine returned so the debug tree can be read on the main thread).
1. Worker loops `step_search`, calling `find_candidates` + `simulate_action`/ `eval_precondition`. When a GDScript callable is needed → `CallbackRequest` on `ctx.request_tx`, return `Pending(id)`, park the node.
1. Main thread each frame: `process_callbacks` (`scheduler.rs:83-233`) — recover finished engines / deliver `Complete` to `agent._on_plan_ready(dict)`, drain `request_rx` and dispatch each request via `dispatch_callback`, resume ready engines, reap done jobs one frame later.

## Backward-chaining search

### Goal initialization (`initialize_goal`, engine.rs:115-154)

Fresh `PlanBranch::new(initial_agent, initial_world)`; each `desired_state` precondition **not** satisfied by the initial state → `(0, pre)` on `open_preconditions` (position 0 = "before the whole chain"). Enqueue.

### Search loop (`step_search`, engine.rs:156-552)

**Phase 1 — resume callbacks**: drain `response_rx`; discovery responses written into caches; parked nodes marked `resumed` with `callback_response`, re-enqueued.

**Phase 2 — main loop**:

- Prune: under BestCost, `heuristic.prune_threshold_met(priority, best_cost)` → re-enqueue + break.
- Budget: iterations > budget → re-enqueue node, return `Pending(0)`.
- Cancellation: `cancel_flag` → `Complete(None)`.
- Visited: fingerprint dedup (skip if seen with ≤ cost).
- **Searching** state: if no open needs → transition to Verifying (sim reset). Depth guard `action_chain.len() >= max_depth` → drop. `find_candidates` → for each ready candidate: dedup via `expanded_candidates`, compute `insert_pos` (earliest consumer), filter the action's own new needs (already open / satisfied by initial state), `insert_action_at`, recalculate cost, cycle guard (`open_requirements.len() > max_depth` → Pruned), enqueue. Park original node if the expander reported a pending callback.
- **Verifying** state → `process_simulation`: Ready → re-enqueue; Pending → park; Invalid → discard (Pruned); Complete → FirstComplete returns immediately, else keep searching.
- Exhaustion: parked → `Pending(first_pending_id)`; empty best plan + more goals → advance goal; else finalize Complete(Some(best_plan)) or failure.

### Candidate discovery (`find_candidates`, expander.rs:147-508)

- Candidate actions: those whose provisions match open requirements via `provision_index` (Fact requirements also check `(FactWildcard, name)`), PLUS all `non_wildcard_actions` (may satisfy preconditions via effects).
- Validity filter against **initial** state: builtins synchronous; customs via `discovery_precond_results` cache → pending/stale-check → fire `EvalCustomPrecond`.
- Two paths:
  - **Wildcard actions**: per requirement × per matching provision; bindings from the requirement (FactWildcard binds the consumer's concrete args); discovery sim; open preconditions evaluated against the simulated post-state.
  - **Non-wildcard actions**: one discovery with empty bindings; emit candidate only if it satisfies ≥1 requirement OR ≥1 precondition.
- `get_discovery_result` (expander.rs:60-132): 3-tier cost cache (`discovery_results` → `discovery_pending`+stale → fire `simulate_action`).

### Forward verification (`process_simulation`, engine.rs:554-859)

1. `sim_index == 0`: `clear_initial_state_requirements` (pos-0 reqs satisfied by initial provisions).
1. Re-check the action's own non-open builtin preconditions against the *current simulated* state (`validate_action_against_current_state`); failure in Verifying → Invalid.
1. Evaluate open preconditions at this position **one at a time**; each satisfied one removed, node re-queued (atomic async processing).
1. Validate requirements via `requirement_holds_in_state` (with bindings).
1. Simulate: cost from `branch_action_costs[sim_index]` / callback response / discovery cache / fire `GetCost`. Effect via `ApplyEffect` callback or identity. On success advance snapshots, `clear_requirements_from_provisions` (concretize wildcards), `sim_index += 1`.
1. End of chain (`finalize_verified_branch`): open needs remain → Invalid; else if `branch.cost < best_cost` record `best_plan`, return Complete.

## Core types (verbatim)

```rust
// plan_types.rs:237-249
#[derive(Clone, Debug)]
pub struct ActionSpec {
    pub name: String,
    pub cost_callable_id: Option<usize>,
    pub effect_callable_id: Option<usize>,
    pub preconditions: Vec<PreconditionSpec>,
    pub validity_checks: Vec<PreconditionSpec>,
    pub requirements: Vec<RequirementSpec>,
    pub provisions: Vec<ProvisionSpec>,
    pub dependent_object_ids: Vec<i64>,
}
```

```rust
// plan_types.rs:252-258
#[derive(Clone, Debug)]
pub struct GoalSpec {
    pub name: String,
    pub reward: f64,
    pub desired_state: Vec<PreconditionSpec>,
    pub original_index: usize,
}
```

```rust
// planner/types.rs:24-50
#[derive(Clone, Debug)]
pub struct PlanBranch {
    pub action_chain: Vec<usize>,
    pub action_costs: Vec<f64>,
    pub action_bindings: Vec<(usize, String, Vec<VariantSnapshot>)>,
    pub open_preconditions: Vec<(usize, PreconditionSpec)>,
    pub open_requirements: Vec<(usize, RequirementSpec)>,
    pub state: BranchState,
    pub goal_index: usize,
    pub simulation_index: usize,
    pub current_agent: BlackboardSnapshot,
    pub current_world: BlackboardSnapshot,
    pub cost: f64,
    pub tree_node_id: usize,
}
```

```rust
// planner/types.rs:118-143
pub struct SearchContext {
    pub actions: Vec<ActionSpec>,
    pub initial_agent: BlackboardSnapshot,
    pub initial_world: BlackboardSnapshot,
    pub initial_provisions: Vec<ProvisionSpec>,
    pub request_tx: Sender<CallbackRequest>,
    pub engine_response_tx: Sender<PlannerCallback>,
    // Discovery caches (thread-safe Mutex'd HashMaps):
    pub discovery_results: ... HashMap<(usize, BindingMap), DiscoveryResult>,
    pub discovery_costs: ... HashMap<(usize, BindingMap), f64>,
    pub discovery_pending: ... HashMap<(usize, BindingMap), usize>,
    pub discovery_request_map: ... HashMap<usize, DiscoveryRequest>,
    pub discovery_precond_results: ... HashMap<(usize, PreconditionSpec, BindingMap), bool>,
    pub discovery_precond_pending: ... HashMap<(usize, PreconditionSpec, BindingMap), usize>,
    pub provision_index: HashMap<(ProvisionKind, String), Vec<usize>>,
    pub non_wildcard_actions: Vec<usize>,
}
```

```rust
// plan_types.rs:260-268
pub struct CallbackRequest {
    pub request_id: usize,
    pub callable_id: usize,
    pub kind: CallbackKind,
    pub bindings: Vec<(String, Vec<VariantSnapshot>)>,
    pub response_tx: Sender<PlannerCallback>,
}
```

```rust
// planner/engine.rs:31-51
pub struct PlannerEngine {
    pub ctx: Arc<SearchContext>,
    pub max_depth: usize,
    pub cancel_flag: Arc<AtomicBool>,
    pub response_rx: Receiver<PlannerCallback>,
    pub response_tx: Sender<PlannerCallback>,
    pub heuristic: Box<dyn SearchHeuristic + Send + Sync>,
    pub termination: TerminationStrategy,
    pub iteration_budget: usize,
    pub queue: BinaryHeap<PriorityNode>,
    pub parked_nodes: HashMap<usize, Vec<SearchNode>>,
    pub visited: HashMap<SearchFingerprint, f64>,
    pub best_plan: Option<PlanResult>,
    pub best_cost: f64,
    pub current_goal_index: usize,
    pub tree: TreeDump,
}
```

Callback kinds (`plan_types.rs:287-306`) / responses (`:310-315`): `GetCost | ApplyEffect | EvalCustomPrecond` → `Float(f64) | Bool(bool) | UpdatedSnapshots(BlackboardSnapshot, BlackboardSnapshot)`.

## Callback mechanism (planner thread ↔ main thread)

- Planner → main: `CallbackRequest { request_id, callable_id, kind, bindings, response_tx }` on the per-job `request_tx` channel.
- Main drain (`process_callbacks`): pull requests, look up callable in `callable_registry`, `dispatch_callback` → send `PlannerCallback` back on `response_tx`.
- Dispatch semantics (`dispatch_callback`, scheduler.rs:702-767):
  - `GetCost`: rebuild blackboards, inject bindings, call `cost_callable(agent, world)`, coerce to f64 (non-numeric → `INFINITY` = action invalid).
  - `ApplyEffect`: same setup, call `effect_callable`, re-snapshot → `UpdatedSnapshots`.
  - `EvalCustomPrecond`: call `eval_callable`, coerce to bool (non-bool → false).
- Bindings injected into agent blackboard **only** (`inject_bindings_into_agent`, scheduler.rs:770-780), using `values[0]`.
- Response consumption: either completes a discovery request (cache write) or resumes a parked node (`callback_response` stored on the node).
- Resume gating: `pending_request_id == 0` (budget yield) always resumable; otherwise resume if response dispatched this frame or already completed (`completed_request_ids`).

## Termination / budget / cancellation

- `TerminationStrategy`: `FirstComplete | BestCost` (default **BestCost** + Dijkstra). BestCost prunes when `priority >= best_cost`; keeps strictly cheaper plans (Dijkstra → optimal). FirstComplete returns first valid plan at verification Complete.
- Budget: `iteration_budget` default 20000, clamped ≥ 100 by scheduler; exceeded → `Pending(0)` (always resumable next frame).
- Depth: `max_depth` clamped ≥ 1; chain-length cut + open-requirements cycle guard both at `max_depth`.
- Cancellation: per-job `Arc<AtomicBool>`; set on re-submit, `cancel_agent_jobs`, `clear_active_jobs`, `cancel_all_jobs`; checked at 3 points in `step_search`. Cancelled jobs return `Complete(None)` without delivery, but the engine is recovered for debug-tree inspection.

## Blackboard snapshot type

- Live `GdPAIBlackboard`: `properties: HashMap<String, Variant>`, `objects: HashMap<String, Gd<SimObjectProxy>>`, `source_objects`. Reserved key `"GDPAI_OBJECTS"` rebuilds `objects` from an array of `GdPAIObjectData` nodes. `clone_for_simulation` deep-copies for branch isolation.
- Send-safe `BlackboardSnapshot` (snapshot.rs:140): `properties: HashMap<String, VariantSnapshot>`, `objects: HashMap<String, SimObjectData>` (`uid`, `groups`, `properties`). Built by `from_blackboard` on the main thread (excludes `GDPAI_OBJECTS` from properties); reconstructed by `into_blackboard`.
- `VariantSnapshot` (snapshot.rs:19): three tiers — Tier 1 primitives (Nil/Bool/Int/Float-u64-bits/Str), Tier 2 `Bytes(var_to_bytes)` (vectors, colors, dicts, resources), Tier 3 `ObjectRef(i64)` live instance handles. `from_variant`/`to_variant` must run on the main thread. Numeric cross-compare via `f64::from_bits` in `snap_as_f64`.

## Notable discrepancies vs docs/PLANNER_PSEUDOCODE.md

- `max_depth` doubles as the open-requirements cycle guard, not just chain length.
- `Pending(0)` is the budget-yield sentinel; scheduler resumes it unconditionally.
- `deferred_action_indices` is vestigial — always `vec![]` wherever populated.
- `serde`/`serde_json` unused in `src/`.
- Greedy requirement clearing lives in `insert_action_at` (types.rs:313-319): one provider clears ALL open requirements with the identical spec.
