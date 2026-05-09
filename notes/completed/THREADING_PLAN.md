# GdPlanningAI — Async / Multi-Agent Planning Brainstorm

## Problem

Planning is fully synchronous on the main thread. A single `RustPlanningEngine.build_plan` call
can run for several milliseconds depending on action/goal count and search depth. With many
agents all re-planning in the same frame (especially in the CONTINUOUS strategy), frame-time
spikes are unavoidable.

The old GDScript engine avoided this via `await` (coroutines), which spread work across frames.
The Rust migration made planning faster but removed that escape hatch.

---

## Root Constraint: GDScript Callables Are Not Thread-Safe

Before evaluating any option it is essential to understand what blocks naive threading.

Inside every planning search step, the Rust engine calls back into GDScript three times per
candidate action:

| Call site | Callable source |
|---|---|
| `action.is_valid(...)` | `PreconditionHandler::eval_callable` (custom preconditions only) |
| `action.get_cost(...)` | `ActionData::cost_callable` — always GDScript |
| `action.apply_effect(...)` | `ActionData::effect_callable` — always GDScript |

These callables hold `Gd<T>` references to GDScript objects. In godot-rs, `Gd<T>` is `!Send`
and `!Sync`. Spawning a `std::thread` and calling any of these from it is **undefined
behaviour** — Godot's object system is not designed for concurrent mutation.

Builtin preconditions (property comparisons) are already pure Rust and are safe to evaluate
from any thread.

---

## Options

### Option A — Frame-Budget Cooperative Scheduler (no threads)

**Concept:** A `GdPAIScheduler` autoload holds a FIFO queue of pending plan requests. In its
`_process`, it runs one request per frame — or as many as fit inside a configurable time budget
(e.g. 2 ms). Agents that request a plan get a signal/callback when the result is ready; they
continue executing their previous plan or idle in the meantime.

> **⚠ Fatal flaw:** The time budget is advisory only. `build_plan` is a synchronous blocking
> call into Rust with no yield point. If a single plan takes 500 ms or several seconds (which
> is realistic in complex game worlds with many actions and deep search trees), the main thread
> is frozen for that entire duration — the scheduler cannot interrupt it mid-execution. The
> frame budget only limits how many plans are *started* per frame, not how long each one runs.

**Architecture:**

```
GdPAIAgent._process
  └── if needs_plan: GdPAIScheduler.submit(self)   # enqueue, do NOT plan here

GdPAIScheduler._process
  └── while budget_remaining and queue not empty:
        request = queue.pop_front()
        result = _bridge.build_plan(...)            # synchronous Rust call
        request.agent.emit_signal("plan_ready", result)

GdPAIAgent (receives signal)
  └── _on_plan_ready(result): apply plan
```

**Pros:**
- Zero threading complexity, zero new Rust code
- Prevents multiple agents from planning in the same frame
- Agents spread across multiple frames naturally
- Works today

**Cons:**
- **A single long plan freezes the entire main thread for its full duration** — no budget
  enforcement is possible once `build_plan` is running
- Cannot exploit multi-core CPUs
- High-priority agents may wait behind low-priority ones (needs priority queue to fix)

**Verdict:** ❌ Not viable as a standalone solution. Useful *only* as a concurrency limiter
layered on top of threading (Options B or C) to prevent flooding the thread pool. The queue
and scheduler infrastructure is still worth building — but planning jobs must be dispatched
to threads, never executed inline on the main thread.

---

### Option B — WorkerThreadPool (GDScript-managed threads)

**Concept:** Use Godot's built-in `WorkerThreadPool` singleton, which pre-allocates worker
threads at startup. `WorkerThreadPool.add_task(callable)` returns an integer task ID; the
callable runs on a worker thread. Completion is polled non-blocking via
`WorkerThreadPool.is_task_completed(task_id)`. `wait_for_task_completion(task_id)` must be
called after completion to free internal resources — but it returns immediately once the task
is already done, so it is safe to call from `_process`.

**Important API constraint — callables don't return values:**
`add_task` discards the callable's return value. Results must be written by the task into a
shared holder object that is accessible to both the task closure and the scheduler.

**Architecture:**

```
# Holder written by the task, read by the scheduler after completion
class PlanResultHolder:
    var result: Dictionary = {}

# Active job tracked by the scheduler
class ActiveJob:
    var task_id: int
    var agent: GdPAIAgent
    var holder: PlanResultHolder

GdPAIScheduler._process:
  # 1. Poll active jobs for completion
  for job in _active_jobs (reverse):
      if WorkerThreadPool.is_task_completed(job.task_id):
          WorkerThreadPool.wait_for_task_completion(job.task_id)  # frees resources
          deliver job.holder.result to job.agent via signal
          remove job

  # 2. Dispatch new jobs up to max_concurrent
  while len(_active_jobs) < max_concurrent and _queue not empty:
      req = _queue.pop()
      snapshot_agent_bb = req.agent.blackboard.clone_for_simulation()   # on main thread
      snapshot_world_bb = req.agent.world_node.get_world_state().clone_for_simulation()
      holder = PlanResultHolder.new()
      task_id = WorkerThreadPool.add_task(func():
          holder.result = req.agent._bridge.build_plan(
              snapshot_agent_bb, snapshot_world_bb,
              req.all_actions, req.goals, req.agent)
      , req.high_priority, "GdPAI:" + req.agent.name)
      _active_jobs.append(ActiveJob{task_id, req.agent, holder})
```

**Key safety requirement — snapshot before submitting:**
The `agent_blackboard` and `world_state` MUST be cloned via `clone_for_simulation()` on the
main thread before the task is submitted. The clone carries only plain `HashMap` data and no
live `Gd<Node>` references, making it safe to access from a worker thread.

**The unresolved concern — Action/Goal callables:**
`Action.get_action_cost`, `Action.simulate_effect`, and `Goal.compute_reward` /
`Goal.get_desired_state` are GDScript methods called from inside the worker thread. This is
technically legal in Godot's threading model *if* the Action/Goal objects are not
simultaneously mutated on the main thread. In practice this holds — planning-time callbacks
only read/write their own cloned blackboard copies — but Godot provides no formal guarantee.
Option C eliminates this concern entirely.

**Pros:**
- True parallel planning across CPU cores
- No Rust changes required — the engine is called normally from the worker thread
- No per-plan thread creation overhead (pool is pre-allocated)
- Non-blocking completion polling — zero main-thread stalling
- `high_priority` flag gives agents a way to skip the queue

**Cons:**
- GDScript callable safety is "works in practice" not "formally guaranteed"
- Actions that accidentally touch the scene tree from cost/effect callbacks will crash or corrupt
- Result holder pattern is more awkward than a direct return value

**Verdict:** The right implementation target for Phase 1. Straightforward GDScript, no Rust
changes, true parallelism. The callable safety concern is manageable by documenting the
"no scene-tree access during planning" rule.

---

### Option C — Rust Channel Bridge (Hybrid Async)

**Concept:** Planning runs on a native Rust background thread (`std::thread::spawn`). When the
planning loop needs to call a GDScript callable, it sends a request over an `mpsc` channel to
the main thread, blocks until the result comes back, then resumes. The main thread processes
these "call requests" in its `_process` loop.

**Architecture:**

```
Main thread                           Background thread
───────────                           ─────────────────
GdPAIScheduler._process               (spawned per planning job or from pool)
  ├── check result_rx for completed    plan_loop():
  │   plans, deliver to agents           for each recursive step:
  │                                        if need GDScript call:
  └── drain call_request_rx:                 call_tx.send(CallRequest)
        result = callable.call(args)         result = result_rx.recv() ← blocks
        call_result_tx.send(result)        else:
                                             pure Rust work (cloning,
                                             precondition eval, tree ops)
```

**What stays on the background thread (pure Rust, no locks needed):**
- Recursive tree traversal
- State cloning (`clone_for_simulation` equivalent as pure Rust HashMap clone)
- Builtin precondition evaluation (already pure Rust)
- Plan tree construction and cost extraction

**What is proxied back to main thread via channel:**
- `cost_callable` calls
- `effect_callable` calls
- `eval_callable` (custom preconditions)

**Serialization problem:** `Gd<T>` is `!Send`, so it cannot be sent over channels. The
`GdPAIBlackboard` itself is `Gd<GdPAIBlackboard>`. This means the background thread cannot
hold a `Gd<GdPAIBlackboard>` at all.

**Workaround:** Replace `Gd<GdPAIBlackboard>` with a pure-Rust `BlackboardSnapshot` struct
(`HashMap<String, VariantSnapshot>` where `VariantSnapshot` is a `Send`-safe enum of
supported value types) for use inside the background thread. The callable proxying sends
snapshots over channels, not `Gd<T>` handles.

**Pros:**
- Truly out-of-main-thread — no main thread blocking during search
- GDScript callbacks remain on the main thread — provably safe
- Works with existing GDScript API
- Scales with core count when multiple agents plan simultaneously

**Cons:**
- Most complex implementation of all options
- If `get_action_cost` / `simulate_effect` are called hundreds of times per plan, channel
  round-trips may negate the threading benefit
- Requires a `Send`-safe mirror of `GdPAIBlackboard` (new Rust types)
- Requires a new scheduler Rust class managing threads and channels
- `VariantSnapshot` must cover all types users put in blackboards

**Verdict:** The correct long-term architecture for genuinely offloading planning. High
implementation cost. Worth pursuing after Option A and B are validated.

---

### Option D — Pure-Rust Serialized Planning (No GDScript Callbacks)

**Concept:** Eliminate GDScript callbacks from the planning hot path entirely. Actions and
goals express their costs and effects as declarative data (key/value mutations, numeric
formulas). The Rust engine plans using only pure Rust data. GDScript callables are only used
at execution time (which already happens on the main thread).

**Declarative action format example:**

```gdscript
# Instead of:
func get_action_cost(agent_bb, world_bb) -> float:
    return agent_bb.get_property("hunger") * 10.0

# Declare as:
func get_action_cost_formula() -> Dictionary:
    return { "type": "multiply", "property": "hunger", "scale": 10.0 }

# Instead of:
func simulate_effect(agent_bb, world_bb) -> void:
    agent_bb.set_property("is_eating", true)

# Declare as:
func get_effect_mutations() -> Array[Dictionary]:
    return [{ "target": "agent", "key": "is_eating", "value": true }]
```

The Rust engine parses these declarative specs. No callbacks during search.

**Pros:**
- Planning is 100% pure Rust — trivially parallelisable with `rayon` (already in Cargo.toml)
- Plans can be reasoned about, cached, serialised to disk
- Maximum performance

**Cons:**
- Breaking API change — existing `simulate_effect` / `get_action_cost` GDScript overrides
  become a migration
- Declarative formulas cannot express complex game logic (inventory lookups, spatial queries,
  anything procedural)
- Removes a key design feature: the ability to write arbitrary GDScript in planning callbacks
- Custom preconditions (`PreconditionCustom`) cannot be expressed declaratively by definition

**Verdict:** Too restrictive. Would remove too much expressivity. Better suited as an opt-in
"fast path" for simple actions rather than a mandatory API change. Could co-exist with the
callable approach as an optimisation.

---

## Recommended Phased Approach

Because a single plan can block the main thread for multiple seconds, **any acceptable
solution must dispatch planning to a non-main thread**. Option A's synchronous-on-main-thread
model is ruled out entirely. The phases below build toward full off-thread planning.

### Phase 1 — `GdPAIScheduler` Autoload + WorkerThreadPool

Build the scheduler and thread dispatch together as the first deliverable. The scheduler
acts as a concurrency limiter (bounded priority queue + max concurrent jobs);
`WorkerThreadPool` does the actual threading. Planning is never called synchronously on the
main thread.

```gdscript
class_name GdPAIScheduler
extends Node

## Maximum number of plans running concurrently on worker threads.
@export var max_concurrent: int = 4

class PlanResultHolder:
    var result: Dictionary = {}

class ActiveJob:
    var task_id: int
    var agent: GdPAIAgent
    var holder: PlanResultHolder

var _queue: Array[PlanRequest] = []     # sorted by priority descending
var _active_jobs: Array[ActiveJob] = []

func submit(agent: GdPAIAgent, priority: int = 0) -> void:
    if not is_instance_valid(agent):
        return
    _queue.append(PlanRequest.new(agent, priority))
    _queue.sort_custom(func(a, b): return a.priority > b.priority)

func _process(_delta: float) -> void:
    # 1. Harvest completed jobs
    for i in range(_active_jobs.size() - 1, -1, -1):
        var job: ActiveJob = _active_jobs[i]
        if WorkerThreadPool.is_task_completed(job.task_id):
            WorkerThreadPool.wait_for_task_completion(job.task_id)  # free resources
            _active_jobs.remove_at(i)
            if is_instance_valid(job.agent):
                job.agent.emit_signal("plan_ready", job.holder.result)

    # 2. Dispatch new jobs up to the concurrency limit
    while _active_jobs.size() < max_concurrent and not _queue.is_empty():
        var req: PlanRequest = _queue.pop_front()
        if not is_instance_valid(req.agent):
            continue
        # Snapshot on main thread BEFORE handing off
        var snap_agent := req.agent.blackboard.clone_for_simulation()
        var snap_world := req.agent.world_node.get_world_state().clone_for_simulation()
        var holder := PlanResultHolder.new()
        var task_id := WorkerThreadPool.add_task(
            func():
                holder.result = req.agent._bridge.build_plan(
                    snap_agent, snap_world,
                    req.all_actions, req.goals, req.agent,
                ),
            req.high_priority,
            "GdPAI:" + req.agent.name
        )
        var job := ActiveJob.new()
        job.task_id = task_id
        job.agent = req.agent
        job.holder = holder
        _active_jobs.append(job)
```

`GdPAIAgent` changes:
- Stop calling `_start_plan()` directly in `_process`
- Instead call `GdPAIScheduler.submit(self)` when a plan is needed
- Add `signal plan_ready(result: Dictionary)` and connect `_on_plan_ready`
- Continue executing current plan while waiting; idle or hold position if no plan exists

Constraints to document for users:
- `get_action_cost` and `simulate_effect` **must not** access the scene tree
- They may only read/write the provided `GdPAIBlackboard` arguments
- `GdPAIObjectData` nodes in world state are accessed via `SimObjectProxy` snapshots
  (pure data, no live Node references) — safe from worker threads

---

### Phase 2 — Pure-Rust Scheduler with Channel Bridge (Option C)

If Phase 1 reveals callable safety issues in practice, or if intra-plan parallelism is
desired, implement the channel bridge:

**New Rust types needed:**
- `BlackboardSnapshot` — `Send`-safe mirror of `GdPAIBlackboard` (plain `HashMap`)
- `GdPAIPlanScheduler` — GDExtension class owning the thread pool and channels
- `PlanJob` — `Send`-able job descriptor (snapshot BBs + serialised action specs)
- `CallbackRequest` / `CallbackResult` — channel messages for GDScript proxy calls

**New GDScript type needed:**
- `GdPAICallbackProxy` — processes `CallbackRequest`s from Rust in `_process`, calls the
  actual GDScript callables, sends results back

This phase makes planning provably safe on background threads and opens the door to `rayon`
parallelism within a single plan (parallel evaluation of sibling branches).

---

---

## Open Questions

1. **Custom precondition callables during background planning.** In Phase 2, custom
   `PreconditionCustom` callbacks are called from the thread. If any of these touch the scene
   tree, they will crash or corrupt. Should custom preconditions be disallowed during background
   planning, or should users declare them as "thread-safe"?

2. **Goal callbacks (`compute_reward`, `get_desired_state`) also call back into GDScript.**
   These are called at plan-start (not during the recursive loop), but still need to be
   snapshotted before the thread starts. Could serialize goal desired-states on the main thread
   before submission.

3. **`rayon` is already in `Cargo.toml` but unused.** It was presumably intended for
   intra-plan parallelism (parallel evaluation of sibling tree branches). This requires
   making `build_plan_recursive` `Send`-safe. In Phase 3, once the channel bridge removes all
   `Gd<T>` from the search path, `rayon::scope` could parallelise the per-action loop in
   `build_plan_recursive` with minimal code changes.

4. **Stale plans.** If an agent's state changes significantly between submitting a plan
   request and receiving the result (especially with a deep queue), the returned plan may be
   invalid. Consider a `plan_generation` counter on the agent: if it changed between submission
   and delivery, discard the result and re-submit.

5. **Plan cancellation.** If an agent is freed or changes goals while waiting in the queue,
   the scheduler should remove its pending request or ignore the result on delivery.
