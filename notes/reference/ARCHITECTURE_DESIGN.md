# GdPlanningAI — Proposed GDScript Architecture

This document describes the target GDScript class design after migration. It specifies each
class's responsibility, its public interface, and how it relates to the Rust layer.

---

## Layer Map

```
┌─────────────────────────────────────────────────────────┐
│  USER GAME CODE                                         │
│  Extends: Action, Goal, GdPAIObjectData,                │
│           GdPAIBehaviorConfig, PropertyUpdater          │
│  Places in scene: GdPAIAgent, GdPAIWorldNode,           │
│                   GdPAIObjectData subclasses            │
│  Creates resources: GdPAIAgentConfig,                   │
│                     GdPAIBehaviorConfig subclasses,     │
│                     GdPAIBlackboardPlan                 │
└─────────────┬───────────────────────────────────────────┘
              │ extends / instantiates
┌─────────────▼───────────────────────────────────────────┐
│  GDSCRIPT FRAMEWORK LAYER (public class_names)          │
│  GdPAIAgent, GdPAIWorldNode, GdPAIObjectData,           │
│  GdPAIInteractable, GdPAILocationData,                  │
│  Action, SpatialAction, Goal,                           │
│  Precondition, PropertyUpdater,                         │
│  GdPAIAgentConfig, GdPAIBehaviorConfig,                 │
│  GdPAIBlackboardPlan                                    │
└─────────────┬───────────────────────────────────────────┘
              │ calls (synchronously)
┌─────────────▼───────────────────────────────────────────┐
│  INTERNAL BRIDGE (no class_name)                        │
│  _gdpai_bridge.gd — serializes Action/Goal/Precondition │
│  objects into Dictionary arrays for the Rust engine     │
└─────────────┬───────────────────────────────────────────┘
              │ GDExtension call
┌─────────────▼───────────────────────────────────────────┐
│  RUST LAYER (GDExtension, read-only from GDScript)      │
│  RustPlanningEngine  — forward-chaining GOAP search     │
│  GdPAIBlackboard     — key/value simulation state       │
│  SimObjectProxy      — world object simulation snapshot │
└─────────────────────────────────────────────────────────┘
```

---

## Public User-Facing Classes

These classes keep their `class_name` and form the stable API surface users write against.

---

### `Action` (refcounteds/action.gd)

**Responsibility:** Define one atomic capability an agent can perform. Bridges the planning world
(Rust) and the execution world (GDScript).

**Planning-time interface** — called from Rust via `Callable`:
```gdscript
func get_validity_checks() -> Array[Precondition]
func get_action_cost(agent_bb: GdPAIBlackboard, world_bb: GdPAIBlackboard) -> float
func get_preconditions() -> Array[Precondition]
func simulate_effect(agent_bb: GdPAIBlackboard, world_bb: GdPAIBlackboard) -> void
```

**Execution-time interface** — called by `GdPAIAgent` during plan execution:
```gdscript
func pre_perform_action(agent: GdPAIAgent) -> Status
func perform_action(agent: GdPAIAgent, delta: float) -> Status
func post_perform_action(agent: GdPAIAgent) -> Status
```

**State helpers** (unchanged):
```gdscript
func set_state(agent: GdPAIAgent, key: String, value: Variant) -> void
func get_state(agent: GdPAIAgent, key: String) -> Variant
func erase_state(agent: GdPAIAgent, key: String) -> void
func has_state(agent: GdPAIAgent, key: String) -> bool
```

**Metadata** (unchanged):
```gdscript
func get_title() -> String
func get_description() -> String
```

**What changes:** Nothing visible to users. The internal bridge calls these methods; the signatures
stay the same. The internal `to_bridge_dict()` method is added for bridge use (see below).

**Internal addition** (not overridden by users):
```gdscript
func _to_bridge_dict() -> Dictionary  # Returns serialized form for the Rust bridge
```

---

### `Goal` (refcounteds/goal.gd)

**Responsibility:** Define what an agent wants to achieve and how much it wants it.

**Interface** (unchanged):
```gdscript
func compute_reward(agent: GdPAIAgent) -> float
func get_desired_state(agent: GdPAIAgent) -> Array[Precondition]
func get_title() -> String
func get_description() -> String
```

**Internal addition:**
```gdscript
func _to_bridge_dict(agent: GdPAIAgent) -> Dictionary
```

---

### `Precondition` (refcounteds/precondition.gd)

**Responsibility:** Represent a condition on blackboard state. Used as goal desired-state
conditions, action preconditions, and validity checks.

**Public interface** — unchanged static factory methods:
```gdscript
static func custom(fn: Callable) -> Precondition
static func agent_has_property(prop: String) -> Precondition
static func agent_property_equal_to(prop, value) -> Precondition
static func agent_property_not_equal_to(prop, value) -> Precondition
static func agent_property_greater_than(prop, value) -> Precondition
static func agent_property_geq_than(prop, value) -> Precondition
static func agent_property_less_than(prop, value) -> Precondition
static func agent_property_leq_than(prop, value) -> Precondition
static func world_state_has_property(prop) -> Precondition
static func world_state_property_equal_to(prop, value) -> Precondition
static func world_state_property_greater_than(prop, value) -> Precondition
static func world_state_property_geq_than(prop, value) -> Precondition
static func world_state_property_less_than(prop, value) -> Precondition
static func world_state_property_leq_than(prop, value) -> Precondition
static func agent_has_object_data_of_group(group) -> Precondition
static func world_state_has_object_data_of_group(group) -> Precondition
static func check_is_object_valid(object) -> Precondition
```

**Virtual interface** used only by the bridge (not overridden by users directly):
```gdscript
func _to_bridge_dict() -> Dictionary  # Implemented by concrete subclasses
```

**What changes:**
- `evaluate(agent, world) -> bool` is **removed from the public interface**. Evaluation now
  happens exclusively inside Rust. GDScript `Precondition` objects exist only to be serialized
  into bridge dictionaries.
- `_do_evaluate` (internal) is removed from `PreconditionBuiltin` since it duplicated Rust logic.
- `PreconditionBuiltin` and `PreconditionCustom` lose their `class_name`, becoming internal
  implementation classes. Users never reference them by name — the static factories return them.

> **Migration note for users:** Any code that calls `precondition.evaluate(bb, ws)` outside of
> planning (e.g., manual checks) must switch to direct blackboard property reads. If standalone
> evaluation is genuinely needed, it should be done through a fresh `RustPlanningEngine.build_plan`
> or by reading blackboard properties directly.

---

### `SpatialAction` (refcounteds/spatial_action.gd)

**Responsibility:** `Action` subclass providing navigation to a `GdPAILocationData` target.
Bundles the "walk to object" behavior so users don't re-implement it for every action.

**Interface** (unchanged from user perspective):
```gdscript
func _init(location: GdPAILocationData, interactable: GdPAIInteractable) -> void
# All Action overrides already provided; users call super() and extend
```

**What changes:**
- Remove the `await` from the nav-agent discovery in `get_validity_checks` (it was never there,
  just confirming it stays synchronous).
- Deduplicate the nav-agent lookup: currently `get_validity_checks` and `pre_perform_action`
  each walk the scene tree looking for a `NavigationAgent2D/3D`. Extract to a shared private
  method `_find_nav_agent(entity) -> Node`.

---

### `PropertyUpdater` (refcounteds/property_updater.gd)

**Responsibility:** Modify agent blackboard properties on every frame.

**Interface** (unchanged):
```gdscript
func initialize(agent: GdPAIAgent) -> void
func update_properties(agent: GdPAIAgent, delta: float) -> void
```

No changes.

---

### `GdPAIAgent` (nodes/gdpai_agent.gd)

**Responsibility:** The agent node placed in the scene. Owns the planning loop and plan execution.

**Public interface** (unchanged):
```gdscript
var blackboard: GdPAIBlackboard       # Agent's own state
var world_node: GdPAIWorldNode        # Reference to scene world node
var goals: Array[Goal]
var self_actions: Array[Action]
func get_current_goal() -> Goal
func get_current_plan() -> Array[Action]
func get_current_plan_step() -> int
func set_planning_strategy(strategy, interval) -> void
func manually_start_plan() -> void
```

**What changes internally:**
1. `_bridge: GdPAIRustBridge` becomes an instance of the internal (no class_name) bridge.
2. `_query_world_state_and_plan()` becomes **fully synchronous** — no `await`.
3. `_compute_valid_self_actions()` is **removed**. The bridge receives all `self_actions` directly;
   Rust's own validity checking handles filtering.
4. `_compute_worldly_actions()` is simplified — no validity-check loop, just collects
   `get_provided_actions()` from all world objects and passes them all to the bridge.
5. `_process` no longer uses `await`. The planning call is a regular synchronous call.

**New planning flow in `_process`:**
```
CONTINUOUS strategy:
  if plan is empty or exhausted:
    _start_plan()     ← synchronous
  _execute_plan(delta)

ON_INTERVAL/FORCED:
  timer callback calls _start_plan() synchronously
  _execute_plan(delta) called from _process
```

---

### `GdPAIWorldNode` (nodes/gdpai_world_node.gd)

**Responsibility:** Provide a shared world `GdPAIBlackboard` to any agent that needs one.

**Interface** (unchanged):
```gdscript
var world_state: GdPAIBlackboard
func get_world_state() -> GdPAIBlackboard
```

No changes needed. Already clean.

---

### `GdPAIObjectData` (nodes/gdpai_object_data.gd)

**Responsibility:** Base node for scene objects that agents can interact with or reason about.

**Interface** (unchanged):
```gdscript
func get_group_labels() -> Array[String]
func get_provided_actions() -> Array[Action]
func get_sim_properties() -> Dictionary
```

No changes. This is the correct interface — Rust calls `get_groups()` and `get_sim_properties()`
via `SimObjectProxy.from_object_data()`.

---

### `GdPAIInteractable` (nodes/gdpai_interactable.gd)

**Responsibility:** Mark an object as physically interactable and store proximity parameters.

**Interface** (unchanged):
```gdscript
@export var max_interaction_distance: float
@export var max_drift_from_plan: float
```

No changes.

---

### `GdPAILocationData` (nodes/gdpai_location_data.gd)

**Responsibility:** Provide `position` and `rotation` for a Node2D or Node3D to the simulation.

**Bug fix required:**
The `position` and `rotation` properties use a setter pattern that causes infinite recursion.
Fix: add `_position` and `_rotation` backing variables used when both node references are null.

```gdscript
# Before (buggy setter):
var position:
    set(val): position = val   # ← infinite recursion

# After:
var _fallback_position: Variant = null
var position:
    set(val): _fallback_position = val
    get:
        if location_node_2d: return location_node_2d.global_position
        if location_node_3d: return location_node_3d.global_position
        return _fallback_position
```

---

### `GdPAIAgentConfig` (resources/gdpai_agent_config.gd)

**Responsibility:** Serializable configuration resource for agent setup.

**What changes:**
- Add `@export var max_recursion: int = 100` so it can be configured in the editor and applied
  to `RustPlanningEngine` at agent startup. This closes the gap seen in test code.

**Updated interface:**
```gdscript
@export var planning_strategy: PlanningStrategy
@export var planning_interval: float
@export var max_recursion: int          # NEW
@export var blackboard_plan: GdPAIBlackboardPlan
@export var behavior_configs: Array[GdPAIBehaviorConfig]
```

---

### `GdPAIBehaviorConfig` (resources/gdpai_behavior_config.gd)

**Responsibility:** Bundle goals, actions, and property updaters that can be attached to an agent.

**Bug fix required:**
Because `GdPAIBehaviorConfig` is a `Resource`, the same `.tres` file instance is shared when
multiple agents use it. The `_is_initialized` guard means `_self_init()` fires only once, so
goals/actions added during `_self_init` are shared and mutated across agents.

Fix: `apply_to_agent` must call `_self_init()` unconditionally (without the cached flag), OR
the agent must duplicate the resource before using it. The cleaner fix is to remove the cache
and call `_self_init()` every time `apply_to_agent()` is called, rebuilding lists fresh each
time. This makes resources stateless.

```gdscript
# After fix: _self_init rebuilds lists every call; _is_initialized removed
func apply_to_agent(agent: GdPAIAgent) -> void:
    var local_goals: Array[Goal] = []
    var local_actions: Array[Action] = []
    var local_updaters: Array[PropertyUpdater] = []
    _populate(local_goals, local_actions, local_updaters)
    agent.goals.append_array(local_goals)
    agent.self_actions.append_array(local_actions)
    for updater in local_updaters:
        updater.initialize(agent)

func _populate(
    goals: Array[Goal],
    actions: Array[Action],
    updaters: Array[PropertyUpdater],
) -> void:
    pass  # Override to add goals/actions/updaters
```

> **Migration note for users:** Rename `_self_init` overrides to `_populate`. The old signature
> of appending to `self.goals`, `self.self_actions`, `self.property_updaters` changes to
> appending to the passed-in arrays.

---

### `GdPAIBlackboardPlan` (gdpai_blackboard_plan.gd)

No changes. Already minimal and correct.

---

## Internal Implementation Classes (no class_name)

These classes are implementation details. They do not appear in user code.

---

### `_GdPAIBridge` (renamed from `gdpai_rust_bridge.gd`)

**Responsibility:** Serialize `Action`, `Goal`, and `Precondition` GDScript objects into
`Array[Dictionary]` structures that `RustPlanningEngine.build_plan` accepts.

**How it changes:**
- Remove `class_name GdPAIRustBridge`.
- `_extract_preconditions` no longer uses `if precond is PreconditionBuiltin` — instead calls
  `precond._to_bridge_dict()` on each precondition. The concrete subclasses produce their own
  dicts.
- `_extract_actions` calls `action._to_bridge_dict()` or its equivalent inline logic. Could
  inline entirely since `Action._to_bridge_dict()` is just constructing the same dict the bridge
  already constructs.
- `build_plan` signature: no longer takes a `GdPAIAgent` object. Takes blackboard + world state
  + action/goal arrays directly, making it a pure serialization + dispatch function with no
  dependency on `GdPAIAgent`.

**New signature:**
```gdscript
func build_plan(
    agent_blackboard: GdPAIBlackboard,
    world_state: GdPAIBlackboard,
    actions: Array[Action],
    goals: Array[Goal],
    agent: GdPAIAgent,  # still needed for goal.compute_reward and goal.get_desired_state
) -> Dictionary
```

---

### Internal `PreconditionBuiltin` and `PreconditionCustom`

Keep as separate files but remove their `class_name` declarations. Add `_to_bridge_dict()`:

```gdscript
# PreconditionBuiltin._to_bridge_dict():
func _to_bridge_dict() -> Dictionary:
    return {
        "target": _target_to_string(target),
        "operation": _operation_to_string(operation),
        "property_name": property,
        "value": value,
    }

# PreconditionCustom._to_bridge_dict():
func _to_bridge_dict() -> Dictionary:
    return {
        "operation": "custom_callback",
        "eval_callable": Callable(self, "evaluate"),
    }
```

The `_target_to_string` and `_operation_to_string` helper conversion methods currently in
`GdPAIRustBridge` move into `PreconditionBuiltin` (or a shared utility).

---

## Rust-Exposed Classes (read-only API, no GDScript changes needed)

These are defined in Rust and consumed by GDScript. No changes.

| Class | Usage |
|---|---|
| `RustPlanningEngine` | Called by the internal bridge. Users do not instantiate this directly. |
| `GdPAIBlackboard` | Passed to action `get_action_cost` / `simulate_effect` during planning. Also the agent's `blackboard` property. |
| `SimObjectProxy` | Returned by `GdPAIBlackboard.get_first_object_in_group` etc. Used inside action simulation. |

---

## File Rename Map

| Current name | New name | Reason |
|---|---|---|
| `gdpai_rust_bridge.gd` | `_gdpai_bridge.gd` | Signals internal-only; no class_name |
| `precondition_builtin.gd` | (unchanged filename, remove class_name) | Stays as impl detail |
| `precondition_custom.gd` | (unchanged filename, remove class_name) | Stays as impl detail |

---

## Dependency Graph (after migration)

```
GdPAIAgent
  ├── GdPAIAgentConfig (resource)
  │     ├── GdPAIBlackboardPlan (resource)
  │     └── GdPAIBehaviorConfig[] (resource, user subclass)
  │           └── Action[], Goal[], PropertyUpdater[]
  ├── GdPAIWorldNode (scene node)
  ├── _GdPAIBridge (internal)
  │     └── RustPlanningEngine (Rust/GDExtension)
  └── GdPAIBlackboard (Rust/GDExtension)

GdPAIWorldNode
  └── GdPAIBlackboard (Rust/GDExtension)

GdPAIObjectData (user subclass)
  ├── get_provided_actions() → Action[]
  └── get_sim_properties() → Dictionary

Action (user subclass)
  └── Precondition (via factory methods)
        ├── [internal PreconditionBuiltin]
        └── [internal PreconditionCustom]

SpatialAction (framework subclass of Action)
  ├── GdPAILocationData
  └── GdPAIInteractable
```
