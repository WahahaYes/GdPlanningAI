# GdPlanningAI — Implementation Plan

This document breaks the migration into discrete, ordered tasks. Tasks are grouped by theme.
Each task is self-contained enough to be reviewed/merged independently.

See `MIGRATION_OVERVIEW.md` for why these changes are needed and `ARCHITECTURE_DESIGN.md`
for the target state of each class.

---

## Task Groups

1. [Bug Fixes](#1-bug-fixes) — fix existing correctness issues
2. [Precondition Refactor](#2-precondition-refactor) — push serialization down, remove class_names
3. [Bridge Internalization](#3-bridge-internalization) — hide the bridge from users
4. [Agent Simplification](#4-agent-simplification) — remove redundancy, drop await
5. [Config Cleanup](#5-config-cleanup) — fix resource sharing, add max_recursion
6. [SpatialAction Cleanup](#6-spatialaction-cleanup) — deduplicate nav-agent lookup
7. [Example Updates](#7-example-updates) — update examples for new BehaviorConfig API
8. [Script Templates](#8-script-templates) — update templates to match new interfaces

---

## 1. Bug Fixes

### 1A. Fix `GdPAILocationData` backing variables

**File:** `scripts/nodes/gdpai_location_data.gd`

**Problem:** The `position` and `rotation` property setters call themselves (infinite recursion),
and the getter fallback also recurses when both node references are null.

**Change:** Introduce `_fallback_position` and `_fallback_rotation` backing variables.

```gdscript
# Remove the old var position / var rotation blocks.
# Replace with:

var _fallback_position: Variant = null
var _fallback_rotation: Variant = null

var position:
    get:
        if location_node_2d != null:
            return location_node_2d.global_position
        if location_node_3d != null:
            return location_node_3d.global_position
        return _fallback_position
    set(val):
        _fallback_position = val

var rotation:
    get:
        if location_node_2d != null:
            return location_node_2d.global_rotation_degrees
        if location_node_3d != null:
            return location_node_3d.global_rotation_degrees
        return _fallback_rotation
    set(val):
        _fallback_rotation = val
```

**Also remove** the `assert` in the getter — it currently asserts that one of the two nodes is
null, but the condition is written incorrectly (`null or null` is always `false`). The correct
guard is an error log if both are non-null. Or simply let the `if` chain handle priority.

---

### 1B. Fix `GdPAIBehaviorConfig` shared-resource bug

**File:** `scripts/resources/gdpai_behavior_config.gd`

**Problem:** `_is_initialized` causes `_self_init` to fire only once. When a `.tres` file is
shared across agents, the second agent skips initialization and inherits the first agent's list
mutations.

**Change:** Remove `_is_initialized`, `goals`, `self_actions`, and `property_updaters` as
instance state. Instead, `apply_to_agent` calls `_populate` with fresh local arrays each time.

```gdscript
class_name GdPAIBehaviorConfig
extends Resource

func apply_to_agent(agent: GdPAIAgent) -> void:
    var local_goals: Array[Goal] = []
    var local_actions: Array[Action] = []
    var local_updaters: Array[PropertyUpdater] = []
    _populate(local_goals, local_actions, local_updaters)
    agent.goals.append_array(local_goals)
    agent.self_actions.append_array(local_actions)
    for updater in local_updaters:
        updater.initialize(agent)

func update_properties(agent: GdPAIAgent, delta: float) -> void:
    pass  # Subclasses that need per-frame updates override this directly
          # or the agent holds PropertyUpdater instances separately (see note below)

func _populate(
    goals: Array[Goal],
    actions: Array[Action],
    updaters: Array[PropertyUpdater],
) -> void:
    pass  # Override to fill arrays
```

> **Note on `update_properties`:** The current design calls `behavior_config.update_properties`
> on every frame in `GdPAIAgent._process`. With `property_updaters` no longer stored on the
> config, `GdPAIAgent` must own the updater list. `apply_to_agent` should append the updaters
> to `agent.property_updaters` (a new array on `GdPAIAgent`), not hold them in the config.
> The agent's `_process` then iterates `property_updaters` directly.

**Updated `GdPAIAgent`** additions to support this:
```gdscript
var property_updaters: Array[PropertyUpdater] = []
```
And in `_process`:
```gdscript
for updater in property_updaters:
    updater.update_properties(self, delta)
```

**User migration:** Rename `_self_init` overrides to `_populate(goals, actions, updaters)` and
append to the provided arrays instead of `self.goals`, `self.self_actions`, etc.

Example before:
```gdscript
func _self_init() -> void:
    super()
    goals.append(SampleHungerGoal.new())
    property_updaters.append(HungerPropertyUpdater.new(hunger_decay, initial_hunger))
```

Example after:
```gdscript
func _populate(goals, actions, updaters) -> void:
    goals.append(SampleHungerGoal.new())
    updaters.append(HungerPropertyUpdater.new(hunger_decay, initial_hunger))
```

---

## 2. Precondition Refactor

### 2A. Add `_to_bridge_dict()` to the `Precondition` base and concrete classes

**Files:**
- `scripts/refcounteds/precondition.gd`
- `scripts/refcounteds/precondition_builtin.gd`
- `scripts/refcounteds/precondition_custom.gd`

**Goal:** The bridge can call `precond._to_bridge_dict()` without inspecting the subtype.

**`precondition.gd` — add abstract method:**
```gdscript
func _to_bridge_dict() -> Dictionary:
    push_error("Precondition._to_bridge_dict must be overridden")
    return {}
```

**`precondition_builtin.gd` — implement + move string conversion helpers here:**
```gdscript
func _to_bridge_dict() -> Dictionary:
    return {
        "target": _target_to_string(target),
        "operation": _operation_to_string(operation),
        "property_name": property,
        "value": value,
    }

static func _target_to_string(t: Target) -> String:
    match t:
        Target.AGENT: return "agent"
        Target.WORLD_STATE: return "world_state"
        _: return "agent"

static func _operation_to_string(op: Op) -> String:
    match op:
        Op.HAS_PROPERTY: return "has_property"
        Op.EQUAL: return "equal"
        Op.NOT_EQUAL: return "not_equal"
        Op.GT: return "greater_than"
        Op.GTE: return "greater_than_or_equal"
        Op.LT: return "less_than"
        Op.LTE: return "less_than_or_equal"
        _: return "has_property"
```

**`precondition_custom.gd` — implement:**
```gdscript
func _to_bridge_dict() -> Dictionary:
    return {
        "operation": "custom_callback",
        "eval_callable": Callable(self, "_do_evaluate"),
    }
```

> **Note on custom callable target:** The custom precondition's `eval_callable` passed to Rust
> must be a bound callable that accepts `(GdPAIBlackboard, GdPAIBlackboard) -> bool`. The
> existing `_do_evaluate(agent, world)` method already has this signature. Binding to
> `_do_evaluate` directly avoids the extra `evaluate → _do_evaluate` indirection.

### 2B. Remove `class_name` from `PreconditionBuiltin` and `PreconditionCustom`

**Files:** `precondition_builtin.gd`, `precondition_custom.gd`

Remove the `class_name` declarations. The static factory methods on `Precondition` return these
types; users never need to reference the type names directly.

**Search for usages first:** Grep for `PreconditionBuiltin` and `PreconditionCustom` across the
codebase to confirm no user-facing code references them by name. Expected usages to remove:
- `gdpai_rust_bridge.gd` — the `if precond is PreconditionBuiltin` check (removed in task 3A)
- `precondition.gd` factory methods — return types can be changed to `Precondition`

**Update factory method return types in `precondition.gd`:**
```gdscript
# Before:
static func agent_has_property(prop: String) -> PreconditionBuiltin:

# After:
static func agent_has_property(prop: String) -> Precondition:
```

### 2C. Remove `_do_evaluate` / `evaluate` from `Precondition`

**File:** `scripts/refcounteds/precondition.gd` and subclasses

With the pre-planning validity pre-filter removed (task 4B), `Precondition.evaluate()` is no
longer called from GDScript. Rust handles all evaluation via the `_to_bridge_dict()` callables.

Remove:
- `Precondition.evaluate(agent, world) -> bool`
- `Precondition._do_evaluate(agent, world) -> bool`
- `PreconditionBuiltin._do_evaluate`
- `PreconditionCustom._do_evaluate` — **KEEP** this one: it is the callable passed to Rust via
  `_to_bridge_dict`. Rename it to `evaluate` to match the Rust-side expectation, or keep as
  `_do_evaluate` and update the callable binding in `_to_bridge_dict`.

> **Caution:** Before removing `evaluate`, confirm no example or user-facing script calls it
> directly. If any do, a deprecation notice pointing to direct blackboard reads is appropriate.

---

## 3. Bridge Internalization

### 3A. Rewrite `gdpai_rust_bridge.gd` to use `_to_bridge_dict()`

**File:** `scripts/gdpai_rust_bridge.gd`

Remove `class_name GdPAIRustBridge`. Rename file to `_gdpai_bridge.gd`.

Rewrite `_extract_preconditions` to call `_to_bridge_dict()`:
```gdscript
func _extract_preconditions(preconditions: Array[Precondition]) -> Array[Dictionary]:
    var extracted: Array[Dictionary] = []
    for precond in preconditions:
        extracted.append(precond._to_bridge_dict())
    return extracted
```

Remove `_target_to_string` and `_operation_to_string` (moved to `PreconditionBuiltin` in 2A).

Update `build_plan` signature to accept blackboard + world state directly, rather than a full
`GdPAIAgent` object. The agent reference is still needed for goal serialization
(`goal.compute_reward(agent)` and `goal.get_desired_state(agent)`):

```gdscript
func build_plan(
    agent_blackboard: GdPAIBlackboard,
    world_state: GdPAIBlackboard,
    actions: Array[Action],
    goals: Array[Goal],
    agent: GdPAIAgent,
) -> Dictionary:
    return planning_engine.build_plan(
        agent_blackboard,
        world_state,
        _extract_actions(actions),
        _extract_goals(goals, agent),
    )
```

### 3B. Update `GdPAIAgent` to use new bridge signature

**File:** `scripts/nodes/gdpai_agent.gd`

Change the bridge instantiation to reference the internal class (no class_name, loaded by path):
```gdscript
const _BridgeClass = preload("res://addons/GdPlanningAI/scripts/_gdpai_bridge.gd")
var _bridge := _BridgeClass.new()
```

Update the `build_plan` call:
```gdscript
var result: Dictionary = _bridge.build_plan(
    blackboard,
    world_node.get_world_state(),
    all_actions,
    goals,
    self,
)
```

---

## 4. Agent Simplification

### 4A. Make `_query_world_state_and_plan` synchronous

**File:** `scripts/nodes/gdpai_agent.gd`

Remove all `await` from this method. The planning call into Rust is synchronous; the validity
pre-filter (which was the source of `await`) is being removed.

```gdscript
func _start_plan() -> void:
    var worldly_actions: Array[Action] = _collect_worldly_actions()
    var all_actions: Array[Action] = []
    all_actions.append_array(self_actions)
    all_actions.append_array(worldly_actions)

    var result: Dictionary = _bridge.build_plan(
        blackboard,
        world_node.get_world_state(),
        all_actions,
        goals,
        self,
    )
    _current_plan_step = -1
    if result.get("success", false):
        _current_action_chain = _bridge.deserialize_plan_result(result, all_actions)
        _current_goal = goals[result.get("goal_index", 0)]
    else:
        _current_action_chain = []
        _current_goal = null
```

Rename `_query_world_state_and_plan` → `_start_plan` to reflect that it is now synchronous.

### 4B. Remove validity pre-filter

**File:** `scripts/nodes/gdpai_agent.gd`

Delete `_compute_valid_self_actions()` entirely.

Simplify `_compute_worldly_actions()` (rename to `_collect_worldly_actions()`):
```gdscript
func _collect_worldly_actions() -> Array[Action]:
    var ws: GdPAIBlackboard = world_node.get_world_state()
    var actions: Array[Action] = []
    var raw_objects = ws.get_property("GDPAI_OBJECTS")
    if raw_objects == null:
        return actions
    for gdpai_object: GdPAIObjectData in raw_objects:
        actions.append_array(gdpai_object.get_provided_actions())
    return actions
```

Note: validity checks are still passed to Rust (they remain on `Action`) — Rust evaluates them
during search. The only change is that GDScript no longer pre-filters before calling Rust.

### 4C. Remove `await` from `_process`

**File:** `scripts/nodes/gdpai_agent.gd`

`_process` currently calls `await _query_world_state_and_plan()`. After making `_start_plan`
synchronous, change the CONTINUOUS strategy block:

```gdscript
func _process(delta: float) -> void:
    for updater in property_updaters:
        updater.update_properties(self, delta)

    if goals.is_empty():
        return

    match _planning_strategy:
        GdPAIAgentConfig.PlanningStrategy.CONTINUOUS:
            if _current_action_chain.is_empty() or _current_plan_step > _current_action_chain.size():
                _start_plan()  # synchronous, no await

    _execute_plan(delta)
```

### 4D. Apply `max_recursion` from config

**File:** `scripts/nodes/gdpai_agent.gd`

In `_ready()`, after creating the bridge / engine, apply `config.max_recursion`:
```gdscript
_bridge.planning_engine.set_max_recursion(config.max_recursion)
```

---

## 5. Config Cleanup

### 5A. Add `max_recursion` to `GdPAIAgentConfig`

**File:** `scripts/resources/gdpai_agent_config.gd`

```gdscript
## Maximum planning search depth. Deeper branches are pruned.
@export var max_recursion: int = 100
```

---

## 6. SpatialAction Cleanup

### 6A. Deduplicate nav-agent lookup

**File:** `scripts/refcounteds/spatial_action.gd`

The pattern of finding `NavigationAgent2D` or `NavigationAgent3D` under an entity appears in
both `get_validity_checks` and `pre_perform_action`. Extract to a shared static helper:

```gdscript
static func _find_nav_agent(entity: Node) -> Node:
    var nav_2d: Node = GdPAIUTILS.get_child_of_type(entity, NavigationAgent2D)
    var nav_3d: Node = GdPAIUTILS.get_child_of_type(entity, NavigationAgent3D)
    assert(nav_2d == null or nav_3d == null, "Entity should not have both 2D and 3D nav agents")
    if nav_2d != null:
        return nav_2d
    return nav_3d
```

Replace the three occurrences of this lookup pattern (in `get_validity_checks`,
`pre_perform_action`, and `sample_wander_action.gd`) with calls to `_find_nav_agent`.

Note: `SampleWanderAction` duplicates this same pattern. If it extends `SpatialAction`, it
inherits the helper automatically. If not (wander doesn't use SpatialAction), the helper could
be moved to `GdPAIUTILS` instead.

### 6B. Cache dist_check per nav agent type in pre_perform_action

Minor cleanup: the `dist_check` magic numbers (`8` for 2D pixels, `0.1` for 3D meters) should
be named constants rather than inline literals.

```gdscript
const ARRIVAL_THRESHOLD_2D: float = 8.0   # pixels
const ARRIVAL_THRESHOLD_3D: float = 0.1   # meters
```

---

## 7. Example Updates

### 7A. Update `HungerBehaviorConfig` for new `_populate` API

**File:** `examples/hunger/hunger_behavior_config.gd`

```gdscript
func _populate(goals: Array[Goal], _actions: Array[Action], updaters: Array[PropertyUpdater]) -> void:
    goals.append(SampleHungerGoal.new())
    updaters.append(HungerPropertyUpdater.new(hunger_decay, initial_hunger))
```

### 7B. Update `WanderBehaviorConfig` for new `_populate` API

**File:** `examples/wander/wander_behavior_config.gd`

```gdscript
func _populate(goals: Array[Goal], actions: Array[Action], _updaters: Array[PropertyUpdater]) -> void:
    goals.append(SampleWanderGoal.new())
    actions.append(SampleWanderAction.new(wander_distance))
```

### 7C. Update `test_rust_bridge.gd` — remove `config.max_recursion`

**File:** `examples/rust_bridge_test/test_rust_bridge.gd`

`config.max_recursion = 4` was setting a field that didn't exist on `GdPAIAgentConfig`. Now that
the field is added (task 5A), this line becomes correct. No code change needed; just verify it
works.

### 7D. Update `SampleWanderAction` nav-agent lookup

**File:** `examples/wander/sample_wander_action.gd`

Replace the inline nav-agent discovery with `SpatialAction._find_nav_agent(entity)` if
`SampleWanderAction` can access `SpatialAction` as a static call, or move the helper to
`GdPAIUTILS`. For now, document as a cleanup opportunity.

---

## 8. Script Templates

**Files:** `script_templates/Action/template.gd`, `script_templates/Goal/template.gd`,
`script_templates/GdPAIBehaviorConfig/template.gd`, `script_templates/GdPAIObjectData/template.gd`

Update templates to reflect the new API:

- `GdPAIBehaviorConfig` template: show `_populate(goals, actions, updaters)` instead of
  `_self_init()`.
- `Action` template: add comment clarifying planning-time vs execution-time methods.
- `Goal` template: no change needed.
- `GdPAIObjectData` template: no change needed.

---

## Implementation Order

The tasks have the following dependencies:

```
1A (LocationData fix)       → independent, do first
1B (BehaviorConfig fix)     → independent, do first
2A (to_bridge_dict)         → must precede 3A
2B (remove class_names)     → must follow 2A, must precede 3A
2C (remove evaluate)        → must follow 4B (pre-filter removed first)
3A (bridge rewrite)         → must follow 2A, 2B
3B (agent uses new bridge)  → must follow 3A
4A (synchronous plan)       → must follow 3B
4B (remove pre-filter)      → must follow 4A
4C (remove await in process)→ must follow 4A
4D (max_recursion)          → must follow 5A
5A (add max_recursion)      → independent
6A (nav-agent dedup)        → independent
6B (dist_check constants)   → independent
7A-7D (examples)            → must follow 1B, 4A
8   (templates)             → must follow 1B
```

**Recommended batch sequence:**
1. `1A`, `1B`, `5A`, `6A`, `6B` — isolated fixes, no cross-dependencies
2. `2A`, `2B` — precondition serialization refactor
3. `3A`, `3B` — bridge internalization
4. `4A`, `4B`, `4C`, `4D` — agent simplification
5. `2C` — remove now-unused `evaluate`
6. `7A`–`7D`, `8` — examples and templates

---

## Testing Checkpoints

After each batch, verify using the existing example scenes:

- **`simple_bridge_test`:** Minimal one-action plan. Should pass with `[TEST PASSED]`.
- **`rust_bridge_test`:** Full hunger + food + spatial planning. Should pass with `[TEST PASSED]`.
- **2D demo (`examples/2D/`):** Visual smoke test — agents should navigate and eat food.

For the `simple_bridge_test`, also verify no `await` appears in the planning path by checking
that `_process` doesn't yield (add a frame counter assert if needed).
