# GdPlanningAI GDScript Redesign Plan

## Overview

**Goal**: Redesign the GDScript layer to be correct, minimal, and frictionless now that a
Rust simulation engine exists. Users should never write Rust, and action/goal authoring
should feel natural and idiomatic in GDScript.

**Scope**: This document covers every layer of the GDScript surface — the bridge protocol,
the public user API (Action, Goal, Precondition), the agent node, and supporting
infrastructure. It incorporates and supersedes `PRECONDITION_REFACTOR_PLAN.md`.

---

## Part 1: Current State & Critical Issues

### 1.1 The Bridge Is Disconnected

`GdPAIAgent._select_highest_reward_goal()` still instantiates the old `Plan.gd` object
directly and never calls `GdPAIRustBridge`. The Rust engine exists and works (per the
example test), but the agent doesn't use it yet. This means:

- All production planning is still happening in GDScript
- `GdPAIRustBridge` is only exercised by the manual example test
- `plan.gd` is treated as the authoritative planner

**Location**: `gdpai_agent.gd:208`
```gdscript
# Current (wrong):
var test_plan: Plan = Plan.new()
test_plan.initialize(self, goals[idx], actions_with_worldly, config.max_recursion)
```

### 1.2 SimulationState ↔ GdPAIBlackboard Format Mismatch (Critical Bug)

`SimulationState.to_dictionary()` outputs a nested format:
```gdscript
{ "properties": { "hunger": 50.0 }, "objects": [ { "uid": "0", ... } ] }
```

But `GdPAIBlackboard` stores a flat internal dict:
```gdscript
{ "hunger": 50.0, "GDPAI_OBJECTS": [<GdPAIObjectData>] }
```

The bridge wrappers (`_create_cost_wrapper`, `_create_effect_wrapper`) reconstruct a
`GdPAIBlackboard` by calling `set_dict(agent_dict)`. This sets the blackboard's internal
dict to the nested format. As a result:

- `blackboard.get_property("hunger")` looks up `_blackboard["hunger"]` → **returns null**
  (the actual value is at `_blackboard["properties"]["hunger"]`)
- `blackboard.get_objects_in_group("GdPAILocationData")` looks up `_blackboard["GDPAI_OBJECTS"]`
  → **returns empty** (the key is `"objects"` in the nested format, not `"GDPAI_OBJECTS"`)

Every action that reads from blackboards in `get_action_cost` or `simulate_effect` is
silently broken. Costs return `0.0` instead of real values. `SpatialAction.get_action_cost`
returns `INF` (can't find `sim_location`) causing all spatial actions to be skipped.

### 1.3 Precondition Mixed Concerns

As documented in `PRECONDITION_REFACTOR_PLAN.md`, the `Precondition` class stores both:
- `eval_func: Callable` — used for custom checks
- `target`, `operation`, `property_name`, `value` — metadata used by the Rust bridge

For built-in preconditions (e.g., `agent_property_greater_than`), a full `Callable` is
constructed and stored even though Rust will evaluate the precondition natively using only
the metadata fields. The callable is redundant.

The bridge then uses an 8-way `if` chain to detect whether a precondition is built-in or
custom (`gdpai_rust_bridge.gd:101-115`). This is fragile and will break silently if a new
operation is added without updating all three places.

### 1.4 GdPAIObjectData Is a Node in Simulation

`GdPAIBlackboard.copy_for_simulation()` creates deep copies of `GdPAIObjectData` nodes
(which extend `Node`, not `RefCounted`). This is:

- **Slow**: Node instantiation is expensive; this happens per recursion level in planning
- **Leaky**: `GdPAIObjectData._init()` calls `add_to_group()`, attaching simulation copies
  to the scene tree's group system
- **Error-prone**: There is a known C++ error that can occur during iteration
  (`gdpai_blackboard.gd:120-121`), requiring a defensive null check

With Rust owning simulation state as `SimulationState` (pure Rust data, no Godot nodes),
object copies should never enter the scene tree.

### 1.5 reverse_simulate_effect Is Dead Code

`plan.gd` uses `reverse_simulate_effect` to backpropagate action effects. The Rust
`planning_engine.rs` does not implement this mechanism — it uses forward simulation with
state cloning instead. Once the agent uses the Rust planner, `reverse_simulate_effect`
becomes unreachable user-facing API. It should be explicitly deprecated.

### 1.6 plan.gd Will Become Dead Code

Once `GdPAIAgent` is wired to use `GdPAIRustBridge`, the entire `Plan` class (246 lines)
is unreachable. It should be removed after the bridge is connected.

### 1.7 Object Simulation Contract Is Undefined for Users

When users override `simulate_effect` or `get_action_cost`, they call:
```gdscript
var sim_location = world_state.get_object_by_uid(object_location.uid)
```
The documented expectation is that `sim_location` is a copy of the object for simulation
use. But there is no clear contract for what type is returned in the bridge callback
context, what methods are available, or whether mutation is propagated back.

### 1.8 Validity Checks Run Before World State Snapshot

`GdPAIAgent._compute_worldly_actions()` and `_compute_valid_self_actions()` both call
`get_validity_checks()` and `await check.evaluate(blackboard, ws_checkpoint)` on the main
thread — before handing off to the Rust bridge. This means:

- Validity checks that need `await` (e.g., async navigation queries) still work
- But they run with a stale world state snapshot (taken at the start of
  `_query_world_state_and_plan()`)
- If planning is multithreaded, validity checks (which may touch scene tree) already ran
  on the main thread — this is actually correct behavior

This is acceptable but should be documented clearly.

---

## Part 2: Design Principles

1. **Users never write Rust** — actions, goals, preconditions, and object data all stay
   in GDScript.

2. **The bridge is invisible** — users should not need to know `GdPAIRustBridge` exists.
   It is internal infrastructure.

3. **Minimal API surface** — remove any method that exists only to support the old GDScript
   planner if it is not needed for Rust.

4. **Duck-typed simulation context** — user methods receive objects that look like
   `GdPAIBlackboard` / `GdPAIObjectData` but are lightweight simulation-time proxies.
   GDScript's dynamic typing means no user code needs to change.

5. **Fail loudly** — remove silent fallbacks (like returning 0.0 for cost when the
   blackboard is reconstructed incorrectly). Bridge errors should produce clear messages.

6. **No simulation copies in the scene tree** — Rust owns simulation state. GDScript
   should not copy Nodes for simulation.

---

## Part 3: Proposed Changes

### 3.1 Precondition Split (from PRECONDITION_REFACTOR_PLAN.md)

Split the monolithic `Precondition` into a class hierarchy. This is the prerequisite for
simplifying the bridge and eliminating the redundant callable storage.

**New class structure:**

```
Precondition          (base, RefCounted)
├─ PreconditionBuiltin   — stores target/op/property/value; no callable
└─ PreconditionCustom    — stores only eval_func callable
```

**`precondition.gd` (base)**:
```gdscript
class_name Precondition
extends RefCounted

var is_satisfied: bool = false

func evaluate(agent: GdPAIBlackboard, world: GdPAIBlackboard) -> bool:
    if is_satisfied:
        return true
    is_satisfied = _do_evaluate(agent, world)
    return is_satisfied

func _do_evaluate(_agent, _world) -> bool:
    push_error("Precondition._do_evaluate must be overridden")
    return false

func copy_for_simulation() -> Precondition:
    push_error("Precondition.copy_for_simulation must be overridden")
    return null
```

**`precondition_builtin.gd`**:
```gdscript
class_name PreconditionBuiltin
extends Precondition

enum Target { AGENT, WORLD_STATE }
enum Op { HAS_PROPERTY, EQUAL, NOT_EQUAL, GT, GTE, LT, LTE }

var target: Target
var operation: Op
var property: String
var value: Variant

func _init(t: Target, op: Op, prop: String, val: Variant = null) -> void:
    target = t; operation = op; property = prop; value = val

func _do_evaluate(agent, world) -> bool:
    var source = agent if target == Target.AGENT else world
    match operation:
        Op.HAS_PROPERTY: return property in source.get_dict()
        Op.EQUAL:        return source.get_property(property) == value
        Op.NOT_EQUAL:    return source.get_property(property) != value
        Op.GT:           return source.get_property(property) > value
        Op.GTE:          return source.get_property(property) >= value
        Op.LT:           return source.get_property(property) < value
        Op.LTE:          return source.get_property(property) <= value
    return false

func copy_for_simulation() -> Precondition:
    var dup = PreconditionBuiltin.new(target, operation, property, value)
    dup.is_satisfied = is_satisfied
    return dup
```

**`precondition_custom.gd`**:
```gdscript
class_name PreconditionCustom
extends Precondition

var eval_func: Callable

func _init(fn: Callable) -> void:
    eval_func = fn

func _do_evaluate(agent, world) -> bool:
    return eval_func.call(agent, world)

func copy_for_simulation() -> Precondition:
    var dup = PreconditionCustom.new(eval_func)
    dup.is_satisfied = is_satisfied
    return dup
```

**Static factory API is unchanged from user's perspective** (keep these on the base class
`Precondition` or a `PreconditionFactory` namespace):
```gdscript
static func agent_property_greater_than(prop, val) -> PreconditionBuiltin:
    return PreconditionBuiltin.new(PreconditionBuiltin.Target.AGENT,
                                   PreconditionBuiltin.Op.GT, prop, val)
static func custom(fn: Callable) -> PreconditionCustom:
    return PreconditionCustom.new(fn)
# ... all existing factory methods remain, return typed PreconditionBuiltin
```

**Bridge extraction simplifies to a type check** (replaces the 8-way if-chain):
```gdscript
func _extract_precondition(precond: Precondition) -> Dictionary:
    if precond is PreconditionBuiltin:
        return {
            "target":        _target_to_string(precond.target),
            "operation":     _op_to_string(precond.operation),
            "property_name": precond.property,
            "value":         precond.value,
            "is_satisfied":  precond.is_satisfied,
        }
    # PreconditionCustom
    return {
        "operation":      "custom_callback",
        "eval_callable":  _wrap_precond_callable(precond),
        "is_satisfied":   precond.is_satisfied,
    }
```

**Backward compatibility**: Keep the old `Precondition.new(callable)` constructor working
during a transition period by having the base class detect the callable and return a
`PreconditionCustom`. After migration, remove it.

---

### 3.2 Fix the Bridge Protocol: Rust-Backed GdPAIBlackboard

#### Why not a parallel SimBlackboard?

The original plan proposed adding a separate `SimBlackboard` Rust class that would mirror
`GdPAIBlackboard`'s API. That creates a drift problem: any method added to `GdPAIBlackboard`
must also be added to `SimBlackboard`, or simulation callbacks silently break.

**Better approach**: Make `GdPAIBlackboard` itself a Rust GDExtension class. There is only
one `GdPAIBlackboard` — the same type used at runtime and in simulation. No drift is
possible because there is nothing to drift against.

`GdPAIBlackboard` is never extended by users (they only consume it), so replacing its
GDScript implementation with Rust imposes no friction on anyone.

---

#### `GdPAIBlackboard` → Rust GDExtension class

```rust
#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct GdPAIBlackboard {
    pub properties: HashMap<String, Variant>,
    pub objects: HashMap<String, Gd<SimObjectProxy>>,
    base: Base<RefCounted>,
}

#[godot_api]
impl GdPAIBlackboard {
    // Exact same API as the current GDScript class
    #[func] fn get_property(&self, key: GString) -> Variant
    #[func] fn set_property(&mut self, key: GString, value: Variant)
    #[func] fn has_property(&self, key: GString) -> bool
    #[func] fn erase_property(&mut self, key: GString)
    #[func] fn get_dict(&self) -> Dictionary

    // Object access — returns SimObjectProxy at all times
    #[func] fn get_objects_in_group(&self, group: GString) -> Array<Gd<SimObjectProxy>>
    #[func] fn get_first_object_in_group(&self, group: GString) -> Variant
    #[func] fn get_object_by_uid(&self, uid: GString) -> Variant

    // Internal: clone the state for a simulation branch
    // Replaces the GDScript copy_for_simulation() — handled entirely in Rust
    pub fn clone_for_simulation(&self) -> Gd<GdPAIBlackboard>
}
```

At runtime, objects are populated by the bridge from the scene's `GdPAIObjectData` nodes
(via `get_sim_properties()`). During planning, Rust clones `GdPAIBlackboard` instances
directly using `clone_for_simulation()` — no GDScript involved, no Node copies.

The planning engine's callback flow becomes straightforward:

```rust
// In planning_engine.rs — no SimBlackboard needed
let sim_agent_bb = agent_blackboard.clone_for_simulation();
let sim_world_bb = world_blackboard.clone_for_simulation();

// GDScript receives actual GdPAIBlackboard objects — same type as at runtime
action.effect_callable.call(&[
    sim_agent_bb.to_variant(),
    sim_world_bb.to_variant(),
]);
// State is already mutated in-place through the Gd<> reference
```

GDScript code in `simulate_effect` and `get_action_cost` receives exactly `GdPAIBlackboard`
objects — the same type they always used. No duck typing required, no type annotation
changes needed anywhere.

---

#### Why not make GdPAIObjectData Rust too?

`GdPAIObjectData` has an inheritance hierarchy that users participate in:

```
GdPAIObjectData  (Node)
├─ GdPAILocationData
├─ GdPAIInteractable
└─ [User subclasses]   ← script template explicitly supports this
```

Making `GdPAIObjectData` a Rust GDExtension class would let users continue extending it
from GDScript (Godot 4 fully supports `class Foo extends RustClass`). However, it doesn't
actually solve the simulation object problem:

- Simulation copies would be **base-class `GdPAIObjectData` instances**, not the user's
  subclass (e.g., not `FoodObject`)
- User-defined properties (`hunger_value`, `quality`) live on the GDScript subclass and
  cannot be on the Rust base class
- `sim_obj.hunger_value` would fail on a sim copy regardless of whether the base is Rust
  or GDScript

The fundamental shift in simulation-time object access — from direct property access
(`obj.hunger_value`) to `get_property("hunger_value")` — is required no matter what. That
shift is best made explicit via a dedicated `SimObjectProxy` type, which makes it clear
you are working with a snapshot, not the live object.

`GdPAIObjectData` stays in GDScript. This preserves the existing extension workflow
entirely and avoids forcing users to extend a Rust class.

---

#### `SimObjectProxy` — the simulation snapshot object

`SimObjectProxy` is a Rust `RefCounted` that holds a snapshot of a `GdPAIObjectData`'s
simulation-relevant state. It lives inside `GdPAIBlackboard` instead of the live Node:

```rust
#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct SimObjectProxy {
    uid: String,
    groups: Vec<String>,
    properties: HashMap<String, Variant>,
    base: Base<RefCounted>,
}

#[godot_api]
impl SimObjectProxy {
    // Core object identity
    #[func] fn uid(&self) -> GString
    #[func] fn is_in_group(&self, group: GString) -> bool
    #[func] fn get_groups(&self) -> Array<GString>

    // Property access for simulation state (replaces direct field access)
    #[func] fn get_property(&self, key: GString) -> Variant
    #[func] fn set_property(&mut self, key: GString, value: Variant)
    #[func] fn has_property(&self, key: GString) -> bool

    // position is special-cased for spatial actions
    #[var] fn get_position(&self) -> Variant   // reads properties["position"]
    #[var] fn set_position(&mut self, p: Variant)  // writes properties["position"]
}
```

`SimObjectProxy` instances are owned directly by `GdPAIBlackboard.objects`. When the
blackboard is cloned for simulation, all proxies are also cloned in Rust — fast, no GDScript
involved.

**Acknowledged drift surface**: `SimObjectProxy` exposes a subset of `GdPAIObjectData`'s
interface. The methods that exist on both must stay in sync (`uid`, `is_in_group`,
`get_groups`, `position`). This is intentional and bounded — these are the only methods
that make sense in simulation context. Adding a new method to `GdPAIObjectData` does NOT
automatically require a `SimObjectProxy` update unless that method is relevant to
simulation.

---

#### How GdPAIBlackboard is populated at runtime

The bridge initializes the blackboard's `objects` map from `GdPAIObjectData` nodes:

```rust
// In GdPAIRustBridge or planning_engine.rs init:
pub fn populate_from_world_state(
    blackboard: &mut GdPAIBlackboard,
    objects: Array<Gd<RefCounted>>,  // GdPAIObjectData nodes
) {
    blackboard.objects.clear();
    for mut obj_gd in objects.iter_shared() {
        if let Some(proxy) = SimObjectProxy::from_object_data(&mut obj_gd) {
            blackboard.objects.insert(proxy.uid.clone(), proxy);
        }
    }
}

impl SimObjectProxy {
    fn from_object_data(obj: &mut Gd<RefCounted>) -> Option<Gd<SimObjectProxy>> {
        let uid = obj.call("uid", &[]).try_to::<String>().ok()?;
        let groups = /* call get_groups() */;
        let mut properties = HashMap::new();

        // Single call — get_sim_properties() is the contract
        if let Ok(sim_props) = obj.call("get_sim_properties", &[]).try_to::<Dictionary>() {
            for (k, v) in sim_props.iter_shared() {
                if let Ok(key) = k.try_to::<String>() {
                    properties.insert(key, v);
                }
            }
        }

        Some(Gd::from_object(SimObjectProxy { uid, groups, properties, base: ... }))
    }
}
```

---

#### What this removes from the bridge

```gdscript
# BEFORE — bridge.gd needs wrapper functions to reconstruct broken blackboards
func _create_cost_wrapper(action: Action) -> Callable: ...   # DELETED
func _create_effect_wrapper(action: Action) -> Callable: ... # DELETED
func _create_precond_eval_wrapper(precond: Precondition) -> Callable: ... # DELETED

# AFTER — direct callables, GdPAIBlackboard passed straight through
func _extract_actions(actions: Array[Action]) -> Array[Dictionary]:
    var extracted: Array[Dictionary] = []
    for action in actions:
        extracted.append({
            "uid":             action.uid,
            "cost_callable":   Callable(action, "get_action_cost"),
            "effect_callable": Callable(action, "simulate_effect"),
            "preconditions":   _extract_preconditions(action.get_preconditions()),
            "validity_checks": _extract_preconditions(action.get_validity_checks()),
        })
    return extracted
```

Rust calls `cost_callable.call([agent_bb, world_bb])` with real `GdPAIBlackboard`
objects. The user's method signature and implementation are unchanged.

---

### 3.3 Wire GdPAIAgent to Use the Rust Bridge

Replace the `Plan.new()` instantiation in `GdPAIAgent` with the `GdPAIRustBridge`.

**Affected method**: `GdPAIAgent._select_highest_reward_goal()`

```gdscript
# Remove:
var test_plan: Plan = Plan.new()
test_plan.initialize(self, goals[idx], actions_with_worldly, config.max_recursion)

# Replace with:
var result: Dictionary = _rust_bridge.build_plan(self, actions_with_worldly, goals)
if result.get("success", false):
    var action_chain = _rust_bridge.deserialize_plan_result(result, actions_with_worldly)
    # wrap in a lightweight PlanResult instead of Plan object
```

The `_rust_bridge` should be a cached instance on `GdPAIAgent` (created in `_ready()`),
not instantiated per planning call.

The `Plan` object is currently used for:
1. Holding the action chain (`get_plan()`)
2. Holding the plan tree for the debugger (`get_plan_tree_debug_data()`)

Both of these need to be preserved after the migration. Options:
- Keep `Plan` as a lightweight result holder (no planning logic, just data)
- Or: return a typed `PlanResult` struct from the bridge

**Recommendation**: Keep the `Plan` class but strip it to a data holder. The bridge
populates it from the Rust `PlanResult`. This preserves the agent's existing execution
code (`_execute_plan`) and the debugger integration unchanged.

```gdscript
# plan.gd becomes a pure data holder:
class_name Plan
extends RefCounted

var _action_chain: Array[Action] = []
var _debug_tree: Dictionary = {}

func set_from_rust_result(result: Dictionary, actions: Array[Action]) -> void:
    # Reconstruct action chain from UIDs
    for uid in result.get("action_chain", []):
        for action in actions:
            if action.uid == uid:
                _action_chain.append(action)
                break
    _debug_tree = result.get("plan_tree", {})

func get_plan() -> Array[Action]:
    return _action_chain

func get_plan_tree_debug_data() -> Dictionary:
    return _debug_tree
```

The 246-line GDScript planning algorithm is deleted. Only the data-holding behavior
remains (roughly 30 lines).

---

### 3.4 Action API Cleanup

#### Remove reverse_simulate_effect

`reverse_simulate_effect` is only called by `plan.gd`'s `_build_plan`. Once `plan.gd`
loses its planning logic (or is removed), this method has no callers.

The Rust engine uses forward simulation with state cloning, making backpropagation
unnecessary. Remove the method from `Action` and all subclasses.

Mark it deprecated first with a one-release warning, then remove.

#### Simplify simulate_effect signature annotation

The docstring for `simulate_effect` should be updated to clarify that during planning,
the blackboard arguments may be `SimBlackboard` objects (not `GdPAIBlackboard`). Since
both expose the same API, user code doesn't need to change, but users should know they
cannot call methods exclusive to `GdPAIBlackboard` (like `copy_for_simulation`) from
within `simulate_effect`.

#### Clarify get_action_cost contract

The existing docstring says "if a reference is invalid, returning INF tells the planner
to skip this action." This contract is correct and should be preserved. However, the note
about `world_state.get_object_by_uid()` returning a simulated version needs updating to
clarify it returns a `SimObjectProxy` during planning.

#### No changes to perform_action / pre_perform_action / post_perform_action

These are called during execution (not planning) and receive the real `GdPAIAgent`. No
changes needed.

---

### 3.5 Goal API Cleanup

The `Goal` base class is minimal (28 lines) and mostly correct. Minor changes:

**Standardize the `get_desired_state` precondition type**: Return
`Array[Precondition]` typed to accept both `PreconditionBuiltin` and `PreconditionCustom`.
Since both extend `Precondition`, no change to the signature is needed, but the docstring
should be updated.

**Add `copy_for_simulation()` awareness**: Goal's `get_desired_state` is called at
planning time. It should not reference scene tree objects that could be freed. The docstring
should warn against capturing live Node references inside lambdas in goal preconditions.

**Consider removing `_agent: GdPAIAgent` parameter from `compute_reward`**: Passing the
full agent is a wide interface. A more targeted option is to pass the `GdPAIBlackboard`.
However, this is a breaking change and lower priority — defer to a later pass.

---

### 3.6 GdPAIObjectData: Eliminate the Duplication Burden

#### The Problem: Every Property Declared Twice

The current simulation copy mechanism forces users to maintain two parallel declarations
for every simulation-relevant property: once as the actual class variable, and once inside
`copy_for_simulation()`. `GdPAILocationData` illustrates this perfectly:

```gdscript
# gdpai_location_data.gd — CURRENT
var position: ...  # declared here ...
var rotation: ...  # declared here ...

func copy_for_simulation() -> GdPAIObjectData:
    var new_data: GdPAIObjectData = GdPAILocationData.new()
    assign_uid_and_entity(new_data)
    new_data.position = position  # ... and duplicated here
    new_data.rotation = rotation  # ... and duplicated here
    return new_data
```

For a user's custom food object:
```gdscript
# CURRENT — user must write copy_for_simulation() manually
class_name FoodObjectData
extends GdPAIObjectData

var hunger_value: float = 10.0
var quality: int = 3
var is_available: bool = true

func copy_for_simulation() -> GdPAIObjectData:
    var copy = FoodObjectData.new()
    assign_uid_and_entity(copy)
    copy.hunger_value = hunger_value  # duplicated
    copy.quality = quality            # duplicated
    copy.is_available = is_available  # duplicated
    return copy
```

If a user adds a new property and forgets to add it to `copy_for_simulation()`, planning
silently uses a stale value. The only indication is incorrect AI behavior.

With Rust owning the simulation state as `SimulationState`, **no GDScript Node copies are
ever created during planning**. The snapshot happens once, in Rust, at the start of
planning. What users need instead is a simple way to tell Rust which properties to
snapshot.

---

#### The New API: `get_sim_properties()`

Replace `copy_for_simulation()` with a single dictionary-returning method:

```gdscript
# GdPAIObjectData base class — new
func get_sim_properties() -> Dictionary:
    return {}
```

Users override this once to list simulation-relevant state. The base class default
(empty dict) is correct for objects that have no simulation-relevant properties beyond
their position.

**`GdPAILocationData` after the change:**
```gdscript
# AFTER — declares properties once, lists them once
func get_sim_properties() -> Dictionary:
    return { "position": position, "rotation": rotation }
```

`copy_for_simulation()` is **deleted** from `GdPAILocationData`.

**User's food object after the change:**
```gdscript
# AFTER
class_name FoodObjectData
extends GdPAIObjectData

var hunger_value: float = 10.0
var quality: int = 3
var is_available: bool = true

func get_sim_properties() -> Dictionary:
    return {
        "hunger_value": hunger_value,
        "quality":      quality,
        "is_available": is_available,
    }
```

`copy_for_simulation()` is **gone entirely**. Property names appear once.

---

#### Why Not Full Automatic Reflection?

GDScript's `get_property_list()` can enumerate all script variables automatically. A
fully automatic approach would look like:

```gdscript
# Hypothetical auto-reflect approach
func get_sim_properties() -> Dictionary:
    var props: Dictionary = {}
    for prop_info in get_property_list():
        if prop_info["usage"] & PROPERTY_USAGE_SCRIPT_VARIABLE:
            var val = get(prop_info["name"])
            if val is bool or val is int or val is float or val is String \
               or val is Vector2 or val is Vector3:
                props[prop_info["name"]] = val
    return props
```

This is tempting but has real problems:
- Includes internal-only bookkeeping variables the user didn't intend to expose
- Silently includes or excludes properties based on type, which is surprising
- Non-primitive types (e.g., arrays of items) are excluded without warning
- Reflection is opaque — users can't tell which properties are being snapshotted

The explicit `get_sim_property_names()` approach is only slightly more work and is always
clear. **Recommended default: explicit list, not reflection.**

---

#### `GdPAILocationData.position` — a Computed Property

`GdPAILocationData.position` is a virtual property with a custom getter that reads from
`location_node_2d.global_position` or `location_node_3d.global_position` at runtime.
The setter stores to a backing variable, used when neither node is set (i.e., in a
simulation copy).

This subtlety means `get_sim_properties()` works correctly as-is:
```gdscript
# Called at planning start on the live object:
# position getter reads from location_node_2d/3d → returns current world position
func get_sim_properties() -> Dictionary:
    return { "position": position, "rotation": rotation }
```

Rust stores `{ "position": Vector2(50, 100) }` in `SimObject.properties["position"]`.
`SimObjectProxy` exposes it as a readable/writable property. During simulation, when
`SpatialAction.simulate_effect` does:
```gdscript
agent_location.position = sim_location.position
```
It writes to `SimObjectProxy.position`, which (via the back-reference described in
section 3.2) updates `SimulationState.objects[uid].properties["position"]`. No Node copy
is involved at any stage.

After planning, `GdPAILocationData` on the real scene is untouched. The sim position
lived and died entirely in Rust memory.

---

#### `SimObjectProxy` — Mutation Propagation via Back-Reference

The `SimObjectProxy` described in section 3.2 uses a `Gd<SimBlackboard>` back-reference
so that property writes propagate immediately to the simulation state:

```rust
pub struct SimObjectProxy {
    uid: String,
    parent: Gd<SimBlackboard>,  // Godot ref-counted pointer — no ownership cycle
    base: Base<RefCounted>,
}

#[godot_api]
impl SimObjectProxy {
    // Reads from parent state
    #[func]
    fn get_property(&self, key: GString) -> Variant {
        self.parent.bind().state
            .objects.get(&self.uid)
            .and_then(|obj| obj.properties.get(&key.to_string()))
            .cloned()
            .unwrap_or(Variant::nil())
    }

    // Writes through to parent state immediately
    #[func]
    fn set_property(&mut self, key: GString, value: Variant) {
        if let Some(obj) = self.parent.bind_mut().state
            .objects.get_mut(&self.uid)
        {
            obj.properties.insert(key.to_string(), value);
        }
    }

    // position is a special-cased @var that delegates to get/set_property("position")
    #[var]
    fn get_position(&self) -> Variant {
        self.get_property("position".into())
    }

    #[var]
    fn set_position(&mut self, pos: Variant) {
        self.set_property("position".into(), pos);
    }
}
```

There is no ownership cycle: `PlanningEngine` holds `Gd<SimBlackboard>`, and
`SimObjectProxy` also holds `Gd<SimBlackboard>`. The `SimBlackboard` doesn't hold
`SimObjectProxy`. Reference count is well-defined and Godot handles cleanup.

---

#### `SimulationState.from_godot` — Updated Extraction

Replace the current `call("get_properties")` / `call("get_global_position")` approach
with a single call to `get_sim_properties()`:

```rust
// simulation_state.rs — updated extract_object
fn extract_object(mut obj: Gd<RefCounted>) -> Option<SimObject> {
    let uid = obj.call("uid", &[]).try_to::<String>().ok()?;

    let groups: Vec<String> = obj.call("get_groups", &[])
        .try_to::<Array<Variant>>().ok()
        .map(|arr| arr.iter_shared().filter_map(|v| v.try_to::<String>().ok()).collect())
        .unwrap_or_default();

    // Single source of truth: get_sim_properties() dict
    let mut properties = HashMap::new();
    let mut position: Option<SimVector3> = None;

    if let Ok(props_dict) = obj.call("get_sim_properties", &[]).try_to::<Dictionary>() {
        for (key, value) in props_dict.iter_shared() {
            if let Ok(key_str) = key.try_to::<String>() {
                // Special-case "position" key into the typed position field
                if key_str == "position" {
                    position = Self::variant_to_sim_vector3(&value);
                } else {
                    properties.insert(key_str, value);
                }
            }
        }
    }

    Some(SimObject { uid, groups, properties, position })
}
```

The fragile `call("get_global_position")` (which returns the Node's scene position, not
`GdPAILocationData`'s logical position) is eliminated. Everything comes through
`get_sim_properties()`.

---

#### Changes to GdPAIBlackboard

Three things on `GdPAIBlackboard` exist solely to manage simulation Node copies and can
be removed:

| Element | Reason for Removal |
|---------|-------------------|
| `is_a_copy: bool` | Guards the cleanup notification; meaningless without Node copies |
| `_notification(PREDELETE)` | Frees simulation `GdPAIObjectData` nodes; no copies = no need |
| `copy_for_simulation()` | Used by `plan.gd` and old simulation path; dead once bridge is live |

`get_provided_actions()` on `GdPAIObjectData` is **not** affected — it runs on the main
thread before planning, not in simulation.

---

#### Impact on SpatialAction

`SpatialAction.get_action_cost()` has two local variables typed as `GdPAILocationData`:

```gdscript
# CURRENT — type hints break with SimObjectProxy
var agent_location: GdPAILocationData = agent_blackboard.get_first_object_in_group(
    "GdPAILocationData",
)
var sim_location: GdPAILocationData = world_state.get_object_by_uid(object_location.uid)
```

With `SimBlackboard`, these return `SimObjectProxy` instances (not `GdPAILocationData`).
Remove the explicit type hints — the `.position` access and `is_instance_valid()` check
both continue to work under duck typing:

```gdscript
# AFTER — remove type hints, behavior identical
var agent_location = agent_blackboard.get_first_object_in_group("GdPAILocationData")
var sim_location = world_state.get_object_by_uid(object_location.uid)
if not is_instance_valid(sim_location):  # null check still works
    return INF
var dist = (agent_location.position - sim_location.position).length()  # still works
```

`SpatialAction.simulate_effect` likewise only needs type hints removed:

```gdscript
# AFTER
var agent_location = agent_blackboard.get_first_object_in_group("GdPAILocationData")
var sim_location = world_state.get_object_by_uid(object_location.uid)
agent_location.position = sim_location.position  # writes through SimObjectProxy
```

---

### 3.8 Multithreading Considerations

The current multithreaded path in `GdPAIAgent` disables thread safety checks on the
worker thread (`thread.set_thread_safety_checks_enabled(false)`). This is necessary
because the old `plan.gd` calls `await` and reads from the scene tree.

With the Rust bridge:
- `GdPAIRustBridge.build_plan()` is a synchronous call (no `await`)
- The Rust engine runs entirely in Rust memory, no scene tree access
- Callbacks to GDScript for `simulate_effect`/`get_action_cost` must be safe

**Constraint**: GDScript callbacks from the worker thread still touch GDScript objects.
Godot's thread-safety model requires that these objects not be modified on the main thread
concurrently. Since the callbacks only operate on the `SimBlackboard` objects (which
exist only on the worker thread), this is safe.

**Recommendation**: Keep the existing thread structure but remove the
`set_thread_safety_checks_enabled(false)` call. If callbacks need to access scene tree
objects (e.g., `SpatialAction` validity checks accessing `NavigationAgent`), those must
remain in the pre-planning validity check phase (main thread), not in the Rust callback.

This means the split is:
- **Validity checks** (`get_validity_checks()`): Run on main thread before handing off
  to Rust. May access scene tree. May `await`.
- **Simulation** (`simulate_effect`, `get_action_cost`): Run in Rust callback context.
  Must not access scene tree. Must not `await`.

This constraint should be clearly documented in the `Action` class docstrings.

---

## Part 4: File-by-File Change Summary

| File | Change | Priority |
|------|--------|----------|
| `precondition.gd` | Split into base class; keep static factory methods; mark `eval_func` constructor deprecated | **P0** |
| `precondition_builtin.gd` | New file | **P0** |
| `precondition_custom.gd` | New file | **P0** |
| `gdpai_rust_bridge.gd` | Simplify extraction (type-check vs if-chain); remove wrapper functions; pass Callables directly | **P0** |
| `rust/src/simulation_state.rs` | Remove `to_dictionary()`/`update_from_dictionary()` (superseded by Rust-backed `GdPAIBlackboard`) | **P0** |
| `rust/src/action.rs` | Remove dict-conversion in `get_cost`/`apply_effect`; pass `GdPAIBlackboard` objects directly | **P0** |
| `rust/src/lib.rs` | Register `GdPAIBlackboard` and `SimObjectProxy` modules | **P0** |
| `rust/src/gdpai_blackboard.rs` | New file — Rust implementation of `GdPAIBlackboard` GDExtension class | **P0** |
| `rust/src/sim_object_proxy.rs` | New file — `SimObjectProxy` GDExtension class | **P0** |
| `gdpai_agent.gd` | Wire `GdPAIRustBridge`; replace `Plan.new()` call; cache bridge instance | **P1** |
| `plan.gd` | Strip to pure data holder; remove all planning algorithm code | **P1** |
| `action.gd` | Deprecate `reverse_simulate_effect`; update docstrings for simulation contract | **P1** |
| `spatial_action.gd` | Remove type hint on `sim_location` variables; update docstrings | **P1** |
| `gdpai_object_data.gd` | Add `get_sim_properties()` hook; delete `copy_for_simulation()` and `assign_uid_and_entity()` | **P1** |
| `gdpai_location_data.gd` | Replace `copy_for_simulation()` with `get_sim_properties()` returning position + rotation | **P1** |
| `gdpai_blackboard.gd` | **Delete** — replaced entirely by `rust/src/gdpai_blackboard.rs` | **P1** |
| `goal.gd` | Update docstrings for simulation contract | **P2** |
| `gdpai_world_node.gd` | No change needed | — |

---

## Part 5: User-Facing API: What Changes, What Doesn't

### Unchanged (users notice nothing)

- `Action`, `SpatialAction`, `Goal` class signatures and all method names
- `Precondition.agent_property_greater_than()` and all factory methods
- `Precondition.new(callable)` — kept for backward compatibility during transition
- `GdPAIAgent` node interface (entity, config, goals, self_actions)
- `GdPAIObjectData`, `GdPAIBlackboard`, `GdPAIWorldNode`, `GdPAIInteractable`
- `perform_action`, `pre_perform_action`, `post_perform_action` on `Action`

### Soft changes (backward compatible with deprecation warnings)

- `Precondition` is now a base class; constructing it directly with a callable is
  deprecated in favor of `Precondition.custom(fn)` → returns `PreconditionCustom`
- `reverse_simulate_effect` deprecated; no-op by default; remove in next major version
- `GdPAIObjectData.copy_for_simulation()` deprecated; will not be called by planner

### Type annotation changes (minimal, only for type-strict projects)

- Local variables in `simulate_effect` and `get_action_cost` that hold objects from
  `get_object_by_uid()` should remove explicit `GdPAIObjectData` / `GdPAILocationData`
  type annotations, since they now receive `SimObjectProxy` during planning. Duck typing
  means the code still runs correctly.

### New API on GdPAIObjectData (replaces a burden)

- `get_sim_properties() -> Dictionary` — override to declare simulation-relevant properties; base returns `{}`
- Replaces `copy_for_simulation()` entirely; properties are declared once instead of twice

### New static factory (additive)

- `Precondition.custom(fn: Callable) -> PreconditionCustom` — preferred spelling for
  custom preconditions

---

## Part 6: Migration Phases

### Phase 0 — Preconditions (isolated, no behavior change)

1. Create `precondition_builtin.gd` and `precondition_custom.gd`
2. Update all `Precondition` static factory methods to return `PreconditionBuiltin`
3. Update `gdpai_rust_bridge.gd` extraction to use type-check
4. Verify the bridge test passes
5. Add deprecation warning to `Precondition.new(callable)`

**Risk**: Low. Built-in preconditions are already described by metadata; the bridge
behavior should be identical.

---

### Phase 1 — SimBlackboard (core bridge fix)

1. Add `sim_blackboard.rs` and `sim_object_proxy.rs` to the Rust crate
2. Register them as GDExtension classes
3. Update `action.rs` to pass `SimBlackboard` to callables instead of dicts
4. Update `gdpai_rust_bridge.gd` to pass `Callable(action, "get_action_cost")` directly
   (remove wrapper functions)
5. Add `get_sim_properties()` to `gdpai_object_data.gd`;
   delete `copy_for_simulation()` and `assign_uid_and_entity()`
6. Replace `copy_for_simulation()` in `gdpai_location_data.gd` with `get_sim_properties()`
7. Update `simulation_state.rs` `extract_object` to call `get_sim_properties()` instead of
   `get_properties()` / `get_global_position`
8. Run the bridge test; verify costs and effects are computed correctly

**Risk**: Medium. This is the largest Rust-side change. The old dict roundtrip is replaced
by object passing.

---

### Phase 2 — Agent Wiring (connect everything)

1. Add `_rust_bridge: GdPAIRustBridge` to `GdPAIAgent._ready()`
2. Rewrite `_select_highest_reward_goal` to call `_rust_bridge.build_plan()`
3. Strip `plan.gd` to a data holder
4. Remove the `await` and `Plan.new()` usage from the agent's planning path
5. Verify multithreaded path works with synchronous bridge call
6. Run integration tests for all examples

**Risk**: Medium. Agent behavior changes; execution logic is untouched but planning path
is fully replaced.

---

### Phase 3 — Cleanup (remove dead code)

1. Remove `plan.gd` planning algorithm (keep data holder or rename to `PlanResult`)
2. Remove `reverse_simulate_effect` from `Action` and all subclasses
3. Remove `copy_for_simulation()` from `GdPAIBlackboard` and `GdPAIObjectData`
4. Remove `is_a_copy` and `_notification` from `GdPAIBlackboard`
5. Remove the `to_dictionary()` / `update_from_dictionary()` roundtrip from Rust
   `SimulationState`
6. Update all docstrings

**Risk**: Low. All removed code is dead by Phase 2.

---

## Part 7: Open Questions

1. **`await` in validity checks**: The current codebase supports `await` in validity
   checks (e.g., navigation reachability). Once the agent is wired to Rust, the validity
   check phase must complete before the synchronous Rust call begins. Confirm this works
   with the existing `await _compute_valid_self_actions()` pattern.

2. **Debugger plan tree format**: `GdPAIAgent._update_debugger_info()` calls
   `_current_plan.get_plan_tree_debug_data()`. The Rust engine's `PlanResult` doesn't
   currently include the debug tree. Either add `plan_tree` to `PlanResult` in Rust, or
   have the stripped `plan.gd` synthesize a simple tree from the action chain.

3. **`GdPAIBehaviorConfig`**: `behavior_configs` are applied to the agent in `_ready()`
   and updated each frame. These are independent of planning and should be unaffected.
   Verify that no behavior configs depend on `Plan` or `Precondition` internals.

4. **`entity: Node` on the agent blackboard**: `GdPAIAgent` stores a `Node` reference
   in the blackboard under `"entity"`. Since `GdPAIBlackboard` is now Rust-backed and
   holds properties as `HashMap<String, Variant>`, storing a `Gd<Node>` variant is valid.
   However, confirm that `clone_for_simulation()` correctly handles this — the simulation
   clone should carry the same entity reference (shallow copy), not attempt to duplicate it.
