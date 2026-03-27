# Option C — Rust Channel Bridge: Targeted Implementation Plan

This document is a single-pass implementation guide. It assumes the current codebase
state described in `ARCHITECTURE_DESIGN.md` and `IMPLEMENTATION_PLAN.md`.

---

## Execution Model

Planning runs on a background `std::thread`. When it needs to call a GDScript callable
(`get_action_cost`, `simulate_effect`, custom preconditions), it sends a `CallbackRequest`
over an `mpsc` channel and blocks on the response. The main thread — inside the scheduler's
`_process` — drains all pending requests each frame in a tight loop, calls the callables,
and sends results back. Both threads advance concurrently across frames.

```
Frame N _process:
  drain callback_request_rx:
    for each request:
      reconstruct GdPAIBlackboard from snapshot   ← main thread only
      call cost_callable / effect_callable          ← main thread only
      serialize result back to snapshot
      send CallbackResponse → background thread unblocks

  check plan_result_rx for completed plans
  deliver to agent via plan_ready signal
```

The main thread never blocks waiting for the background thread. It processes whatever
is pending and returns. If the background thread is mid-Rust-work and has nothing queued,
the loop exits immediately. Planning advances as quickly as the 60fps callback window
allows — typically completing within a small number of frames for reasonably-sized planners.

---

## Files to Create

```
rust/src/snapshot.rs          — Send-safe Variant/Blackboard/SimObject types
rust/src/background_types.rs  — Send-safe action/goal/precondition specs + channel messages
rust/src/background_plan.rs   — background planning algorithm (works on snapshots)
rust/src/scheduler.rs         — GdPAIPlanScheduler GDExtension class
```

## Files to Modify

```
rust/src/lib.rs               — register GdPAIPlanScheduler
rust/src/planning_engine.rs   — extract pure-Rust helpers for reuse
addons/GdPlanningAI/gdpai_autoload.gd          — add scheduler node
addons/GdPlanningAI/scripts/nodes/gdpai_agent.gd — submit to scheduler instead of sync plan
```

---

## 1. `snapshot.rs` — Send-safe Data Types

### `VariantSnapshot`

Three tiers, in evaluation order:

**Tier 1 — primitive fast path** (`bool`, `int`, `float`, `String`): stored as plain Rust
values. These are the types used in builtin precondition comparisons. The background thread
evaluates them directly without any channel round-trip.

**Tier 2 — `var_to_bytes` fallback**: Godot's built-in binary serialiser handles
`Vector2`, `Vector3`, `Color`, `Rect2`, `Transform2D/3D`, `Array`, `Dictionary`,
typed packed arrays, and any `Resource` subclass. The result is a `Vec<u8>` — trivially
`Send`. On reconstruction (main thread only) `bytes_to_var` restores the original type
with full fidelity.

**Tier 3 — live `Object` references**: Non-resource `Object` instances cannot be serialised
by `var_to_bytes`. However, the background thread only ever *carries* Object values — it
never calls methods on them. Only the main-thread GDScript callables (`get_action_cost`,
`simulate_effect`) actually use the objects. This means we don't need to serialise the
object at all — we only need to store a stable **handle** that the main thread can resolve
back to the real object at callback time.

Godot's `instance_id()` is that handle: it is a stable integer unique to each live object.
The background thread carries `ObjectRef(id)` opaquely. When `dispatch_callback`
reconstructs a `GdPAIBlackboard` for a callable, it calls `Gd::try_from_instance_id(id)`
to get the real object back.

```
Snapshot (main thread):  Object → ObjectRef(instance_id)
Background thread:        carries ObjectRef(id) as an opaque i64 — never dereferences it
Reconstruction (main):    ObjectRef(id) → Gd::try_from_instance_id(id) → real Object
```

**Branch-isolation caveat:** When the planner clones a blackboard for a simulation branch,
`ObjectRef` copies are *shallow* — all branches share the same live Godot object via the
same ID. This means **Object references cannot be branch-isolated**. If two branches call
`simulate_effect` on callables that mutate the same object, they will interfere. For
anything requiring per-branch isolation, use `SimObjectProxy` / `GDPAI_OBJECTS`. `ObjectRef`
is safe for read-only objects (config tables, databases) or objects that callables *replace*
wholesale (`agent_bb.set_property("target", different_object)`) rather than mutate in place.

> **User-defined GDScript classes** round-trip automatically if they extend `Resource`
> (Tier 2 via `var_to_bytes`). Plain `RefCounted` subclasses that are not `Resource` fall
> into Tier 3 as `ObjectRef`. They work correctly in callables but cannot be branch-isolated.

```rust
#[derive(Clone, Debug)]
pub enum VariantSnapshot {
    // Tier 1 — primitives (used by builtin precondition evaluation on background thread)
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    // Tier 2 — all other serialisable Godot types (Vector2, Color, Array, Dictionary,
    //           Resource subclasses, etc.) encoded via var_to_bytes on the main thread.
    Bytes(Vec<u8>),
    // Tier 3 — live Object references stored as Godot instance IDs.
    //           The background thread carries these opaquely; only the main thread
    //           resolves them back to real objects in dispatch_callback.
    ObjectRef(i64),
}

impl VariantSnapshot {
    /// Construct from a Variant. Must be called on the main thread.
    pub fn from_variant(v: &Variant) -> Self {
        if v.is_nil()                        { return Self::Nil; }
        if let Ok(b) = v.try_to::<bool>()   { return Self::Bool(b); }
        if let Ok(i) = v.try_to::<i64>()    { return Self::Int(i); }
        if let Ok(f) = v.try_to::<f64>()    { return Self::Float(f); }
        if let Ok(s) = v.try_to::<String>() { return Self::Str(s); }

        // Tier 2: attempt Godot's binary serialiser
        let bytes = godot::global::var_to_bytes(v.clone());
        if !bytes.is_empty() {
            return Self::Bytes(bytes.to_vec());
        }

        // Tier 3: live Object — store instance ID as opaque handle
        if let Ok(obj) = v.try_to::<Gd<Object>>() {
            return Self::ObjectRef(obj.instance_id().to_i64());
        }

        // Truly unhandled — should not normally occur
        log_warn!(
            "VariantSnapshot: could not snapshot value of type {:?}; storing as Nil.",
            v.get_type()
        );
        Self::Nil
    }

    /// Reconstruct to a Variant. Must be called on the main thread.
    pub fn to_variant(&self) -> Variant {
        match self {
            Self::Nil         => Variant::nil(),
            Self::Bool(b)     => b.to_variant(),
            Self::Int(i)      => i.to_variant(),
            Self::Float(f)    => f.to_variant(),
            Self::Str(s)      => s.to_variant(),
            Self::Bytes(b)    => {
                let packed = godot::builtin::PackedByteArray::from(b.as_slice());
                godot::global::bytes_to_var(packed)
            }
            Self::ObjectRef(id) => {
                let instance_id = godot::obj::InstanceId::from_i64(*id);
                match Gd::<Object>::try_from_instance_id(instance_id) {
                    Ok(obj) => obj.to_variant(),
                    Err(_)  => {
                        log_warn!("ObjectRef({}): object was freed before callback; returning Nil.", id);
                        Variant::nil()
                    }
                }
            }
        }
    }
}
```

### `SimObjectData`

Pure-Rust equivalent of `SimObjectProxy`. Replaces `Gd<SimObjectProxy>` on the background
thread.

```rust
#[derive(Clone, Debug)]
pub struct SimObjectData {
    pub uid: String,
    pub groups: Vec<String>,
    pub properties: HashMap<String, VariantSnapshot>,
}
```

### `BlackboardSnapshot`

Pure-Rust equivalent of `GdPAIBlackboard`. The background thread operates on these
exclusively — it never holds a `Gd<GdPAIBlackboard>`.

```rust
#[derive(Clone, Debug)]
pub struct BlackboardSnapshot {
    pub properties: HashMap<String, VariantSnapshot>,
    pub objects: HashMap<String, SimObjectData>,
}

impl BlackboardSnapshot {
    /// Snapshot a live GdPAIBlackboard on the main thread.
    pub fn from_blackboard(bb: &GdPAIBlackboard) -> Self {
        let properties = bb.properties.iter()
            .map(|(k, v)| (k.clone(), VariantSnapshot::from_variant(v)))
            .collect();
        let objects = bb.objects.iter()
            .map(|(uid, proxy)| {
                let p = proxy.bind();
                let obj = SimObjectData {
                    uid: p.uid.clone(),
                    groups: p.groups.clone(),
                    properties: p.properties.iter()
                        .map(|(k, v)| (k.clone(), VariantSnapshot::from_variant(v)))
                        .collect(),
                };
                (uid.clone(), obj)
            })
            .collect();
        Self { properties, objects }
    }

    /// Reconstruct a GdPAIBlackboard from a snapshot (main thread only).
    pub fn into_blackboard(self) -> Gd<GdPAIBlackboard> {
        let mut bb = GdPAIBlackboard::new_gd();
        {
            let mut b = bb.bind_mut();
            b.properties = self.properties.iter()
                .map(|(k, v)| (k.clone(), v.to_variant()))
                .collect();
            for (uid, obj_data) in self.objects {
                let mut proxy = SimObjectProxy::new_gd();
                {
                    let mut p = proxy.bind_mut();
                    p.uid = obj_data.uid;
                    p.groups = obj_data.groups;
                    p.properties = obj_data.properties.iter()
                        .map(|(k, v)| (k.clone(), v.to_variant()))
                        .collect();
                }
                b.objects.insert(uid, proxy);
            }
        }
        bb
    }

    /// Re-snapshot a GdPAIBlackboard after a callable has mutated it.
    pub fn from_blackboard_after_mutation(bb: Gd<GdPAIBlackboard>) -> Self {
        Self::from_blackboard(&bb.bind())
    }
}
```

`BlackboardSnapshot` is trivially `Clone` and all its fields are `Send`, so the compiler
derives `Send` automatically — no `unsafe` required.

---

## 2. `background_types.rs` — Send-safe Specs and Channel Messages

### `PreconditionSpec`

Replaces `PreconditionHandler` on the background thread. Builtin variants carry their data
directly; custom variants carry a `callable_id` into the scheduler's registry.

```rust
#[derive(Clone, Debug)]
pub enum PreconditionSpec {
    Builtin {
        target: PreconditionTarget,  // re-use existing enum (Clone+Send)
        operation: PreconditionOp,   // re-use existing enum (Clone+Send)
        property_name: String,
        value: Option<VariantSnapshot>,
    },
    Custom {
        callable_id: usize,
    },
}

impl PreconditionSpec {
    /// Evaluate a builtin precondition against snapshots (no channel needed).
    pub fn evaluate_builtin(
        &self,
        agent: &BlackboardSnapshot,
        world: &BlackboardSnapshot,
    ) -> Option<bool> {
        // Same logic as PreconditionHandler::evaluate but over BlackboardSnapshot
        // Returns None if this is a Custom variant (caller must use channel)
        match self {
            Self::Builtin { target, operation, property_name, value } => {
                let source = match target {
                    PreconditionTarget::Agent => agent,
                    PreconditionTarget::WorldState => world,
                };
                Some(eval_builtin_on_snapshot(operation, property_name, value.as_ref(), source))
            }
            Self::Custom { .. } => None,
        }
    }

    pub fn callable_id(&self) -> Option<usize> {
        match self { Self::Custom { callable_id } => Some(*callable_id), _ => None }
    }
}
```

> **Note on `PreconditionTarget` / `PreconditionOp`:** These enums in `precondition.rs`
> do not hold any `Gd<T>` — they are `Clone + Send` today. They can be re-used directly.

### `ActionSpec` and `GoalSpec`

```rust
#[derive(Clone, Debug)]
pub struct ActionSpec {
    pub name: String,
    pub cost_callable_id: usize,
    pub effect_callable_id: usize,
    pub preconditions: Vec<PreconditionSpec>,
    pub validity_checks: Vec<PreconditionSpec>,
}

#[derive(Clone, Debug)]
pub struct GoalSpec {
    pub name: String,
    pub reward: f64,  // already a plain float — no callback needed
    pub desired_state: Vec<PreconditionSpec>,
    pub original_index: usize,
}
```

### Channel Messages

```rust
/// Sent from background thread → main thread.
pub struct CallbackRequest {
    pub callable_id: usize,
    pub kind: CallbackKind,
    pub response_tx: std::sync::mpsc::Sender<CallbackResponse>,
}

pub enum CallbackKind {
    /// Returns float cost. Snapshots are read-only.
    GetCost {
        agent: BlackboardSnapshot,
        world: BlackboardSnapshot,
    },
    /// Returns updated snapshots after effect is applied.
    ApplyEffect {
        agent: BlackboardSnapshot,
        world: BlackboardSnapshot,
    },
    /// Returns bool result for a custom precondition.
    EvalCustomPrecond {
        agent: BlackboardSnapshot,
        world: BlackboardSnapshot,
    },
}

/// Sent from main thread → background thread.
pub enum CallbackResponse {
    Float(f64),
    Bool(bool),
    /// Mutated snapshots returned after ApplyEffect.
    UpdatedSnapshots(BlackboardSnapshot, BlackboardSnapshot),
}
```

All types above are `Send` because they contain only `String`, `f64`, `bool`,
`HashMap<String, VariantSnapshot>`, and `mpsc::Sender<T: Send>`.

---

## 3. `background_plan.rs` — Background Planning Algorithm

This module re-implements the planning algorithm from `planning_engine.rs` but operates
entirely on `BlackboardSnapshot` and `ActionSpec`, calling GDScript only via the channel.

```rust
use std::sync::mpsc::{Sender, Receiver};
use crate::background_types::*;
use crate::planning_engine::{PlanResult, PlanTreeNode};
use crate::snapshot::BlackboardSnapshot;

pub fn run_plan(
    mut agent: BlackboardSnapshot,
    mut world: BlackboardSnapshot,
    actions: Vec<ActionSpec>,
    goals: Vec<GoalSpec>,
    request_tx: Sender<CallbackRequest>,
    result_tx: Sender<PlanResult>,
) {
    let mut sorted_goals = goals;
    sorted_goals.sort_by(|a, b| b.reward.partial_cmp(&a.reward).unwrap_or(std::cmp::Ordering::Equal));

    for goal in sorted_goals {
        if is_goal_satisfied_snap(&goal.desired_state, &agent, &world, &request_tx) {
            let _ = result_tx.send(PlanResult {
                success: true, action_chain: vec![], total_cost: 0.0,
                goal_index: goal.original_index as i64,
            });
            return;
        }

        let mut root = PlanTreeNode {
            action_index: -1, cost: 0.0,
            desired_state: vec![], children: vec![],
        };

        if build_recursive(&mut root, &agent, &world, &actions, 0, 100, &request_tx) {
            let plan = extract_best_plan(&root);
            let _ = result_tx.send(PlanResult {
                success: true,
                action_chain: plan.actions,
                total_cost: plan.cost,
                goal_index: goal.original_index as i64,
            });
            return;
        }
    }

    let _ = result_tx.send(PlanResult::failure());
}

fn call_get_cost(
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

fn call_apply_effect(
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
        _ => panic!("apply_effect callback failed"),
    }
}

fn eval_precondition_spec(
    spec: &PreconditionSpec,
    agent: &BlackboardSnapshot,
    world: &BlackboardSnapshot,
    request_tx: &Sender<CallbackRequest>,
) -> bool {
    match spec.evaluate_builtin(agent, world) {
        Some(result) => result,
        None => {
            // Custom precondition — proxy via channel
            let callable_id = spec.callable_id().unwrap();
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
    }
}
```

> **Re-use `extract_best_plan` and `find_lowest_cost_path`:** Move these functions from
> `planning_engine.rs` to a new `plan_tree.rs` module (or make them `pub` free functions).
> Both work on `PlanTreeNode` which contains no `Gd<T>` and can be shared.

---

## 4. `scheduler.rs` — `GdPAIPlanScheduler` GDExtension Class

Rayon's thread pool (`rayon` is already in `Cargo.toml`) is used instead of bare
`std::thread::spawn`. `pool.spawn(closure)` is fire-and-forget — no `JoinHandle` is
returned. Completion is detected via the result channel, same as before. The pool is
built once in `INode::ready` after Godot has applied exported properties.

```rust
use std::sync::mpsc::{Receiver, Sender};
use crate::background_types::*;
use crate::planning_engine::PlanResult;

// No JoinHandle — Rayon manages thread lifetimes internally.
struct ActiveJobHandle {
    agent: Gd<Node>,
    request_rx: Receiver<CallbackRequest>,
    result_rx: Receiver<PlanResult>,
    done: bool,
}

#[derive(GodotClass)]
#[class(base=Node)]
pub struct GdPAIPlanScheduler {
    /// 0 = use Rayon default (number of logical CPUs).
    #[export] max_threads: i64,
    callable_registry: Vec<Callable>,
    active_jobs: Vec<ActiveJobHandle>,
    thread_pool: Option<rayon::ThreadPool>,
    base: Base<Node>,
}

#[godot_api]
impl INode for GdPAIPlanScheduler {
    fn init(base: Base<Node>) -> Self {
        Self {
            max_threads: 0,
            callable_registry: Vec::new(),
            active_jobs: Vec::new(),
            thread_pool: None,
            base,
        }
    }

    fn ready(&mut self) {
        let mut builder = rayon::ThreadPoolBuilder::new();
        if self.max_threads > 0 {
            builder = builder.num_threads(self.max_threads as usize);
        }
        self.thread_pool = Some(builder.build().expect("GdPAIPlanScheduler: failed to build thread pool"));
    }
}

#[godot_api]
impl GdPAIPlanScheduler {
    /// Called every frame by GDScript. Drains callback requests and delivers results.
    #[func]
    fn process_callbacks(&mut self) {
        for job in self.active_jobs.iter_mut().filter(|j| !j.done) {
            // Drain ALL pending callback requests this frame
            loop {
                match job.request_rx.try_recv() {
                    Ok(req) => self.dispatch_callback(req),
                    Err(_) => break,
                }
            }
            // Check for completed plan
            if let Ok(result) = job.result_rx.try_recv() {
                job.done = true;
                if is_instance_valid(&job.agent) {
                    let mut dict = result_to_dict(&result);
                    job.agent.call("_on_plan_ready", &[dict.to_variant()]);
                }
            }
        }
        // Clean up finished jobs — no join needed, Rayon owns the threads.
        self.active_jobs.retain(|job| !job.done);
    }

    /// Submit a planning job from GDScript.
    ///
    /// `actions` and `goals` are the same Array[Dictionary] format as build_plan.
    #[func]
    fn submit_plan(
        &mut self,
        agent: Gd<Node>,
        agent_bb: Gd<GdPAIBlackboard>,
        world_bb: Gd<GdPAIBlackboard>,
        actions: Array<VarDictionary>,
        goals: Array<VarDictionary>,
        high_priority: bool,
    ) {
        // 1. Snapshot blackboards on main thread
        let snap_agent = BlackboardSnapshot::from_blackboard(&agent_bb.bind());
        let snap_world = BlackboardSnapshot::from_blackboard(&world_bb.bind());

        // 2. Register callables and build ActionSpecs
        let action_specs = self.build_action_specs(actions);
        let goal_specs = self.build_goal_specs(goals);

        // 3. Create channels
        let (req_tx, req_rx) = std::sync::mpsc::channel::<CallbackRequest>();
        let (res_tx, res_rx) = std::sync::mpsc::channel::<PlanResult>();

        // 4. Dispatch to Rayon thread pool — fire and forget, no handle returned.
        self.thread_pool
            .as_ref()
            .expect("submit_plan called before ready()")
            .spawn(move || {
                crate::background_plan::run_plan(
                    snap_agent, snap_world, action_specs, goal_specs, req_tx, res_tx,
                );
            });

        self.active_jobs.push(ActiveJobHandle {
            agent,
            request_rx: req_rx,
            result_rx: res_rx,
            done: false,
        });
    }
}

impl GdPAIPlanScheduler {
    fn dispatch_callback(&self, req: CallbackRequest) {
        let callable = &self.callable_registry[req.callable_id];
        let response = match req.kind {
            CallbackKind::GetCost { agent, world } => {
                let bb_agent = agent.into_blackboard();
                let bb_world = world.into_blackboard();
                let result = callable.call(&[bb_agent.to_variant(), bb_world.to_variant()]);
                let cost = result.try_to::<f64>().unwrap_or(f64::INFINITY);
                CallbackResponse::Float(cost)
            }
            CallbackKind::ApplyEffect { agent, world } => {
                let bb_agent = agent.into_blackboard();
                let bb_world = world.into_blackboard();
                callable.call(&[bb_agent.to_variant(), bb_world.to_variant()]);
                // Re-snapshot the (now mutated) blackboards
                let new_agent = BlackboardSnapshot::from_blackboard(&bb_agent.bind());
                let new_world = BlackboardSnapshot::from_blackboard(&bb_world.bind());
                CallbackResponse::UpdatedSnapshots(new_agent, new_world)
            }
            CallbackKind::EvalCustomPrecond { agent, world } => {
                let bb_agent = agent.into_blackboard();
                let bb_world = world.into_blackboard();
                let result = callable.call(&[bb_agent.to_variant(), bb_world.to_variant()]);
                CallbackResponse::Bool(result.try_to::<bool>().unwrap_or(false))
            }
        };
        let _ = req.response_tx.send(response);
    }

    fn build_action_specs(&mut self, actions: Array<VarDictionary>) -> Vec<ActionSpec> {
        actions.iter_shared().filter_map(|dict| {
            let name = dict.get("name")?.try_to::<String>().ok()?;
            let cost_callable = dict.get("cost_callable")?.try_to::<Callable>().ok()?;
            let effect_callable = dict.get("effect_callable")?.try_to::<Callable>().ok()?;

            let cost_id = self.register_callable(cost_callable);
            let effect_id = self.register_callable(effect_callable);

            let preconditions = self.extract_precond_specs(&dict, "preconditions");
            let validity_checks = self.extract_precond_specs(&dict, "validity_checks");

            Some(ActionSpec { name, cost_callable_id: cost_id, effect_callable_id: effect_id,
                              preconditions, validity_checks })
        }).collect()
    }

    fn build_goal_specs(&self, goals: Array<VarDictionary>) -> Vec<GoalSpec> {
        goals.iter_shared().enumerate().filter_map(|(idx, dict)| {
            let name = dict.get("name")?.try_to::<String>().ok()?;
            let reward = dict.get("reward")?.try_to::<f64>().ok()?;
            let desired_state = self.extract_precond_specs_from_dict(&dict, "desired_state");
            Some(GoalSpec { name, reward, desired_state, original_index: idx })
        }).collect()
    }

    fn register_callable(&mut self, callable: Callable) -> usize {
        let id = self.callable_registry.len();
        self.callable_registry.push(callable);
        id
    }

    fn extract_precond_specs(&mut self, dict: &VarDictionary, key: &str) -> Vec<PreconditionSpec> {
        // Convert each precondition Dictionary to PreconditionSpec.
        // Custom callables are registered and replaced with callable_id.
        dict.get(key)
            .and_then(|v| v.try_to::<Array<VarDictionary>>().ok())
            .map(|arr| arr.iter_shared().filter_map(|d| self.precond_spec_from_dict(&d)).collect())
            .unwrap_or_default()
    }

    fn precond_spec_from_dict(&mut self, dict: &VarDictionary) -> Option<PreconditionSpec> {
        let op_str = dict.get("operation")?.try_to::<String>().ok()?;
        if op_str == "custom_callback" {
            let callable = dict.get("eval_callable")?.try_to::<Callable>().ok()?;
            let id = self.register_callable(callable);
            return Some(PreconditionSpec::Custom { callable_id: id });
        }
        // Builtin — same parsing as PreconditionHandler::from_dict but into VariantSnapshot
        let target = /* parse "target" field */;
        let operation = /* parse op_str */;
        let property_name = dict.get("property_name")
            .and_then(|v| v.try_to::<String>().ok())
            .unwrap_or_default();
        let value = dict.get("value").map(|v| VariantSnapshot::from_variant(&v));
        Some(PreconditionSpec::Builtin { target, operation, property_name, value })
    }
}
```

> **Callable Registry lifetime:** Callables are registered per `submit_plan` call and held
> alive in `callable_registry` until the job completes. The registry grows monotonically;
> IDs are indices so lookup is O(1). On job cleanup, entries can be cleared or the registry
> can be rebuilt per-job (simpler). Per-job registry is safer (no stale IDs across jobs).

---

## 5. Changes to Existing Files

### `src/lib.rs`

```rust
pub mod snapshot;
pub mod background_types;
pub mod background_plan;
pub mod scheduler;
pub mod plan_tree;   // extracted from planning_engine.rs
```

Register `GdPAIPlanScheduler`:
```rust
// The gdextension macro auto-registers all GodotClass types, so
// just declaring the module is sufficient.
```

### `src/planning_engine.rs`

Extract `extract_best_plan` and `find_lowest_cost_path` into `plan_tree.rs` so they can
be called from both `planning_engine.rs` and `background_plan.rs`. The existing synchronous
`build_plan` is **not removed** — it stays as the non-threaded path for backward compat.

### `gdpai_autoload.gd`

Add the scheduler as a child node:

```gdscript
var _scheduler: GdPAIPlanScheduler

func _ready() -> void:
    _apply_log_level()
    EngineDebugger.send_message("gdplanningai:clear_state", [])
    _scheduler = GdPAIPlanScheduler.new()
    add_child(_scheduler)

func _process(_delta: float) -> void:
    _scheduler.process_callbacks()

func get_scheduler() -> GdPAIPlanScheduler:
    return _scheduler
```

### `scripts/nodes/gdpai_agent.gd`

Add async planning path. The synchronous `_start_plan` stays for the `FORCED` strategy.
The `CONTINUOUS` and `ON_INTERVAL` strategies use the scheduler:

```gdscript
signal plan_ready(result: Dictionary)

func _start_plan_async() -> void:
    var all_actions: Array[Action] = []
    all_actions.append_array(self_actions)
    all_actions.append_array(_collect_worldly_actions())

    GdPAIAutoload.get_scheduler().submit_plan(
        self,
        blackboard,
        world_node.get_world_state(),
        _bridge.serialize_actions(all_actions, self),
        _bridge.serialize_goals(goals, self),
        false,
    )
    _waiting_for_plan = true

func _on_plan_ready(result: Dictionary) -> void:
    _waiting_for_plan = false
    if result.get("success", false):
        _current_action_chain = _bridge.deserialize_plan_result(result, _last_submitted_actions)
        _current_goal = goals[result.get("goal_index", 0)]
    else:
        _current_action_chain = []
        _current_goal = null
    _current_plan_step = -1

func _process(delta: float) -> void:
    for updater in property_updaters:
        updater.update_properties(self, delta)

    if goals.is_empty() or _waiting_for_plan:
        _execute_plan(delta)  # keep running current plan while waiting
        return

    match _planning_strategy:
        GdPAIAgentConfig.PlanningStrategy.CONTINUOUS:
            if _current_action_chain.is_empty() or _current_plan_step >= _current_action_chain.size():
                _start_plan_async()
        # ON_INTERVAL and FORCED still call _start_plan() synchronously
        # (or can be migrated to async in a follow-up)

    _execute_plan(delta)
```

---

## 6. Snapshot Fidelity and Known Limitations

### `GDPAI_OBJECTS` special-casing

The `GDPAI_OBJECTS` blackboard property is an `Array` of live `GdPAIObjectData` nodes.
`BlackboardSnapshot` represents these as `objects: HashMap<String, SimObjectData>` instead.
The `GDPAI_OBJECTS` key is **excluded** from the `properties` map in the snapshot — objects
are accessed only through `objects`.

When reconstructing a `GdPAIBlackboard` in `dispatch_callback` (for `ApplyEffect`):
- Properties are restored from `VariantSnapshot` via `to_variant()` (see tier table below)
- Objects are restored from `SimObjectData` as fresh `SimObjectProxy` instances
- The reconstructed blackboard has no `source_objects` map — identical to
  `clone_for_simulation()` today, which is correct for planning callbacks

### Type coverage summary

| Blackboard value type | Round-trips? | Branch-isolated? | Notes |
|---|---|---|---|
| `bool`, `int`, `float`, `String` | ✅ full | ✅ yes | Tier 1; also comparable in builtin preconditions |
| `Vector2/3`, `Color`, `Rect2`, typed arrays | ✅ full | ✅ yes | Tier 2 via `var_to_bytes` |
| `Array`, `Dictionary` | ✅ full | ✅ yes | Tier 2 via `var_to_bytes` |
| `Resource` subclass | ✅ full | ✅ yes | Tier 2 via `var_to_bytes` |
| `RefCounted` subclass (non-Resource) | ✅ callable sees real object | ❌ shared across branches | Tier 3 `ObjectRef`; callable receives original instance |
| `Node` / live scene object | ✅ callable sees real object | ❌ shared across branches | Tier 3 `ObjectRef`; use `SimObjectProxy` if branch-isolation needed |

### Only remaining limitation

Tier 3 (`ObjectRef`) values are **shallow** in cloned simulation branches. All branches
carry the same instance ID and therefore resolve to the same live object in `dispatch_callback`.
If `simulate_effect` for two parallel branches both mutate an `ObjectRef` value in-place,
they will interfere. The fix is to use `SimObjectProxy` / `GDPAI_OBJECTS` for any world
object that needs per-branch state isolation.

---

## 7. Open Items Before Implementation

1. ~~Thread pool~~ — **resolved:** `rayon::ThreadPool` is used (see `scheduler.rs`).
   Pool is built in `INode::ready` with `max_threads` export (0 = CPU count). Rayon
   queues jobs internally when all threads are busy, so the scheduler never blocks.

2. **Callable registry per job vs. global.** Per-job is described above (safest). A global
   registry with refcounting is possible but adds complexity. Start per-job.

3. **Thread safety of `Callable`/`Gd<T>` in `callable_registry`.** The registry is only
   accessed from the main thread (in `dispatch_callback`, called from `_process`). The
   background thread never touches it — it only sends IDs. This is safe.

4. **`_bridge.serialize_actions` / `serialize_goals`** methods need to be exposed so the
   agent can pass the serialized form to `submit_plan`. These already exist inside the
   bridge — they just need extracting as standalone methods that return
   `Array<VarDictionary>` without calling `build_plan` immediately.

5. **`_last_submitted_actions` tracking.** `_on_plan_ready` needs to reconstruct the
   action chain from indices into the action array that was submitted. The agent must
   store the submitted action list between `_start_plan_async()` and `_on_plan_ready()`.

6. **Stale plan detection.** If the agent's goal changes or the agent is freed while a
   plan is in flight, the scheduler's `process_callbacks` already handles freed agents
   via `is_instance_valid`. For goal changes, add a `_plan_generation: int` counter
   incremented on each `_start_plan_async` call; check it in `_on_plan_ready`.
