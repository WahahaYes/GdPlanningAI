# GDScript Core Audit

**Scope:** All GDScript in `addons/GdPlanningAI/scripts/`, `examples/`, and `test/`.
**Date:** 2026-07-04

---

## Critical Bugs

### 1. `deserialize_plan_result` Never Calls `clone_for_plan` — Action State Corruption

**File:** `addons/GdPlanningAI/scripts/gdpai_rust_bridge.gd:32-35`

```gdscript
for chain_position in range(result.action_chain.size()):
    var action_index: int = result.action_chain[chain_position]
    var action: Action = actions[action_index]
    action_chain.append(action)
```

The bridge reuses the SAME `Action` instance from the submitted actions array for every occurrence in the plan. If an action appears multiple times (e.g., `GoToAction` used to visit two different targets), the second occurrence overwrites `target_location` (and any other instance state) set by the first.

**Impact:** Plans with repeated actions execute incorrectly. The `GoToAction` template has `clone_for_plan()` but it is never invoked.

**Fix:** In `deserialize_plan_result`, call `action.clone_for_plan()` before appending. If `clone_for_plan()` returns `self` (default), that's the current behavior. Actions that need isolation can override.

---

### 2. `GoToAction` Does Not Override `clone_for_plan`

**File:** `addons/GdPlanningAI/scripts/refcounteds/goto_action.gd`

`GoToAction` stores `target_location` as mutable instance state. The default `clone_for_plan` in `Action` returns `self`. Since the bridge doesn't call it anyway (see #1), even if `GoToAction` implemented cloning, it wouldn't help until the bridge is fixed.

**Fix:** Override `clone_for_plan` in `GoToAction` to return a new `GoToAction` with the same nav agent reference but `target_location = null`. Then fix #1.

---

### 3. `world_node` Null Dereference Crash

**File:** `addons/GdPlanningAI/scripts/nodes/gdpai_agent.gd:160,282`

```gdscript
world_node.get_world_state()  # line 160 in _start_plan_async
world_node.get_world_state()  # line 282 in _collect_worldly_actions
```

If no `GdPAIWorldNode` exists in the scene, `world_node` remains `null`. Planning or action execution crashes with a null reference error.

**Fix:** Add null guards:
```gdscript
if world_node == null:
    push_warning("GdPAIAgent: no GdPAIWorldNode found in scene")
    _waiting_for_plan = false
    return
```

---

### 4. `ShakeTreeAction._init` — Null Dereference Risk

**File:** `examples/objects/fruit_tree/shake_tree_action.gd:25-28`

```gdscript
var fruit: Node = fruit_tree.fruit_prefab.instantiate()
fruit.queue_free()
var food_item: FoodObject = GdPAIUTILS.get_child_of_type(fruit, FoodObject)
_sim_hunger_gain = food_item.hunger_value * fruit_tree.drop_min_amount
```

`food_item` could be null if the prefab doesn't contain a `FoodObject` child. Accessing `food_item.hunger_value` would crash.

**Fix:** Add null check:
```gdscript
if food_item == null:
    push_error("ShakeTreeAction: fruit prefab has no FoodObject child")
    _sim_hunger_gain = 0.0
else:
    _sim_hunger_gain = food_item.hunger_value * fruit_tree.drop_min_amount
```

---

## Medium Severity Issues

### 5. `_execute_plan` Post-Actions Are a Guaranteed Cleanup Phase

**File:** `addons/GdPlanningAI/scripts/nodes/gdpai_agent.gd:257-275`

When an action returns `FAILURE`, `_current_plan_step` is set to `action_chain.size()`. On the next frame, the post-action block triggers and calls `post_perform_action` on ALL actions, including those that never ran. This is the intended design: `post_perform_action` is a guaranteed cleanup phase that must be safe to call even if `pre_perform_action` or `perform_action` returned `FAILURE` or were never invoked.

**Fix:** Document the cleanup contract clearly in `Action.post_perform_action` and in the agent's post-action block so action authors implement it correctly.

---

### 6. `PreconditionBuiltin` Enum Documentation Malformed

**File:** `addons/GdPlanningAI/scripts/refcounteds/precondition_builtin.gd:10-13`

```gdscript
enum Target { AGENT, WORLD_STATE }  ## The agent's blackboard  ## The world state blackboard
```

Godot's documentation parser does not support multiple `##` doc comments on a single enum line. Only the first comment is attached to the enum declaration; individual values get no docs.

**Fix:** Use inline documentation per value (Godot 4.2+):
```gdscript
enum Target {
    AGENT,        ## The agent's blackboard
    WORLD_STATE,  ## The world state blackboard
}
```

Same issue exists for the `Op` enum.

---

### 7. `GoToAction` Binding Flattening is Fragile

**File:** `addons/GdPlanningAI/scripts/refcounteds/goto_action.gd:60-70,125-135`

Both `get_action_cost` and `simulate_effect` contain identical nested-array-flattening logic:

```gdscript
while flattened.size() > 0 and flattened[0] is Array:
    flattened = flattened[0]
```

This suggests bindings are sometimes double-wrapped in arrays during bridge serialization. The flattening is defensive but masks a data contract issue. If bindings were pre-injected correctly (as they are now in the Rust scheduler), this flattening may be unnecessary.

**Fix:** Verify whether double-wrapping still occurs after the recent bridge changes. If not, remove the flattening logic and use `binding` directly.

---

### 8. `GdPAIAgent._on_plan_ready` — Goal Index Not Bounds-Checked

**File:** `addons/GdPlanningAI/scripts/nodes/gdpai_agent.gd:182`

```gdscript
_current_goal = goals[result.get("goal_index", 0)]
```

If the Rust scheduler ever returns an out-of-bounds `goal_index` (bug in scheduler), this crashes. A defensive bounds check would prevent GDScript from crashing due to a Rust-side bug.

**Fix:**
```gdscript
var goal_index: int = result.get("goal_index", 0)
if goal_index >= 0 and goal_index < goals.size():
    _current_goal = goals[goal_index]
else:
    push_error("GdPAIAgent: goal_index %d out of bounds (goals.size()=%d)" % [goal_index, goals.size()])
    _current_goal = null
```

---

### 9. `entity` Export Used Without Null Guard

**File:** `addons/GdPlanningAI/scripts/nodes/gdpai_agent.gd:53-55`

```gdscript
blackboard.set_property("entity", entity)
var agent_objects: Array = GdPAIUTILS.get_children_in_group(entity, "GdPAIObjectData")
```

If `entity` is left unassigned in the editor, `get_children_in_group(entity, ...)` receives null and may crash depending on `GdPAIUTILS` implementation.

**Fix:** Guard:
```gdscript
if entity != null:
    var agent_objects: Array = GdPAIUTILS.get_children_in_group(entity, "GdPAIObjectData")
    blackboard.set_property("GDPAI_OBJECTS", agent_objects)
```

---

## Low Severity / Polish

### 10. `Action.set_state` Fallback to "0" for Negative Chain Position

**File:** `addons/GdPlanningAI/scripts/refcounteds/action.gd:111-130`

When `chain_position < 0`, state keys all fall back to prefix `"0"`. If two different Action instances both have `chain_position == -1` and the same `get_instance_id()`... wait, `get_instance_id()` is per-instance, so they won't collide. This is actually safe. Minor concern only.

---

### 11. `GdPAIWorldNode.get_world_state` Returns Mutable Shared Reference

**File:** `addons/GdPlanningAI/scripts/nodes/gdpai_world_node.gd:14-19`

Returns the internal `@onready var world_state` directly. Callers can mutate it. Since the Rust scheduler snapshots the blackboard at submission time, this doesn't cause planner bugs, but it's poor encapsulation.

**Fix:** Return a copy or document that callers should not mutate.

---

### 12. `GdPAIAgentConfig` — Clamp Values Not Documented

**File:** `addons/GdPlanningAI/scripts/resources/gdpai_agent_config.gd:18-20`

`max_recursion` and `iteration_budget` are silently clamped by the Rust scheduler to minimums of 1 and 100. Users might set `max_recursion = 0` expecting unlimited depth and get 1 instead without warning.

**Fix:** Add `@export_range` or doc comments noting the clamps. (The scheduler now logs warnings, but the resource docs should also mention it.)

---

## Architecture Observations

### 13. `_collect_worldly_actions` Rebuilds Action List Every Plan

**File:** `addons/GdPlanningAI/scripts/nodes/gdpai_agent.gd:281-289`

Every planning cycle iterates all world objects and collects their provided actions. This is O(world objects) per plan. For small scenes it's fine, but for many agents and objects it could be expensive.

**Observation:** Not a bug, but an optimization opportunity. Could cache until objects change.

---

### 14. `Precondition.check_is_object_valid` Captures Variant, Not Object

**File:** `addons/GdPlanningAI/scripts/refcounteds/precondition.gd:203-210`

```gdscript
static func check_is_object_valid(object: Variant) -> Precondition:
    var deps: Array[Object] = []
    if object is Object:
        deps.append(object)
    var check: Callable = func(_b, _w) -> bool:
        return is_instance_valid(object)
    return PreconditionCustomWithDeps.new(check, deps)
```

The lambda captures `object` (a Variant). If `object` is a Node that gets freed, `is_instance_valid(object)` returns false, which is correct. The dependency list also handles it. This is actually well-designed.

---

### 15. `PreconditionCustom.to_bridge_dict` Creates New Lambda Every Call

**File:** `addons/GdPlanningAI/scripts/refcounteds/precondition_custom.gd:40-46`

```gdscript
func to_bridge_dict() -> Dictionary:
    var eval: Callable = func(a, w) -> bool:
        return _do_evaluate(a, w)
    return {
        "operation": "custom_callback",
        "eval_callable": eval,
    }
```

A new Callable is allocated every serialization. Since planning serializes actions every frame (for continuous planning), this allocates a lambda per custom precondition per frame. For a few preconditions it's fine, but for many agents it could be noticeable.

**Observation:** The lambda could be cached as a member variable, but this is a micro-optimization.

---

## Test Observations

### 16. `test_requirements_provisions.gd` — Inline Action Dictionaries Duplicated Heavily

The test file defines inline action dictionaries with full keys (`cost_callable`, `effect_callable`, `preconditions`, etc.) repeated in every test. This is ~30-50 lines of boilerplate per test. A helper to build action dicts would reduce duplication.

**Observation:** Code health, not a functional issue.

---

## Verified Correct

| Feature | Status | Notes |
|---|---|---|
| `Goal.compute_reward` dynamic re-evaluation | ✅ | Called every plan submission via `_extract_goals` |
| `BehaviorConfig.apply_to_agent` isolation | ✅ | Builds fresh lists per call; safe for shared resources |
| `Action` state isolation via `chain_position` | ✅ | Keys prefixed with instance ID + position |
| `PreconditionCustomWithDeps` object validation | ✅ | Checks `is_instance_valid` before invoking callable |
| `GdPAIAgent._inflight_generation` stale result discard | ✅ | Correctly rejects outdated plan results |
| `GoToAction` nav agent 2D/3D detection | ✅ | Asserts mutual exclusion, handles both correctly |
| `RequirementSpec` / `ProvisionSpec` bridge dicts | ✅ | Match Rust-side `from_dict` expectations |
| `PreconditionBuiltin` target/op string mapping | ✅ | Matches Rust `PreconditionHandler::parse_operation` |

---

## Summary of Recommended Fixes

| Priority | Issue | File | Action |
|---|---|---|---|
| **Critical** | Missing `clone_for_plan` in deserialization | `gdpai_rust_bridge.gd` | Call `clone_for_plan()` per occurrence |
| **Critical** | `GoToAction` doesn't clone | `goto_action.gd` | Override `clone_for_plan` |
| **Critical** | `world_node` null crash | `gdpai_agent.gd` | Add null guards |
| **Critical** | `ShakeTreeAction._init` null risk | `shake_tree_action.gd` | Add null check for `food_item` |
| **Medium** | Post-actions run on abort | `gdpai_agent.gd` | Document or guard with abort flag |
| **Medium** | Enum docs malformed | `precondition_builtin.gd` | Restructure per-value docs |
| **Medium** | Goal index unchecked | `gdpai_agent.gd` | Add bounds check |
| **Medium** | `entity` null dereference | `gdpai_agent.gd` | Guard `get_children_in_group` |
| **Low** | Binding flattening mystery | `goto_action.gd` | Investigate and simplify if possible |
| **Low** | Config clamp docs | `gdpai_agent_config.gd` | Document minimum values |

---

## Subagent Prompt Seeds

> **Working directory:** All `make` and `cargo` commands must be run from the project root. Prefix every command with `cd /c/godot/gdplanningai &&` (or `cd C:/Godot/GdPlanningAI &&` on Windows) so the correct `Cargo.toml`, `Makefile`, and `.gutconfig.json` are found. Do not rely on the shell's default directory.

---

### Seed A — Fix Action Cloning in Bridge + GoToAction `clone_for_plan`

**Files:** `addons/GdPlanningAI/scripts/gdpai_rust_bridge.gd`, `addons/GdPlanningAI/scripts/refcounteds/goto_action.gd`

**Task:** Fix action instance reuse so repeated actions in a plan don't corrupt each other's state.

1. In `deserialize_plan_result` (`gdpai_rust_bridge.gd`), before appending an action to `action_chain`, call `action.clone_for_plan()`. Use the result of that call (which may be `self` for actions that don't need isolation).
2. In `GoToAction` (`goto_action.gd`), override `clone_for_plan()` to return a new `GoToAction` instance with the same `entity`/`nav_agent` reference but `target_location = null`. The planner bindings will be re-injected into the cloned instance at execution time.

**Do NOT touch:** The Rust scheduler, `Action` base class, or any other action subclasses.

**Acceptance criteria:** `make test-godot` passes. A plan containing two `GoToAction` occurrences (e.g., go to A then go to B) keeps independent `target_location` values.

---

### Seed B — Null Safety Guards

**Files:** `addons/GdPlanningAI/scripts/nodes/gdpai_agent.gd`, `examples/objects/fruit_tree/shake_tree_action.gd`

**Task:** Add null guards to prevent runtime crashes from missing nodes or malformed prefabs.

1. In `gdpai_agent.gd` `_start_plan_async` (around line 160): before calling `world_node.get_world_state()`, check `if world_node == null`. If null, log a `push_warning`, set `_waiting_for_plan = false`, and return early.
2. In `gdpai_agent.gd` `_ready` (around line 53-55): before calling `GdPAIUTILS.get_children_in_group(entity, ...)`, check `if entity != null`. If null, skip the object collection.
3. In `gdpai_agent.gd` `_collect_worldly_actions` (around line 282): guard `world_node.get_world_state()` with a null check; return empty array if `world_node == null`.
4. In `shake_tree_action.gd` `_init` (around line 25-28): after `get_child_of_type(fruit, FoodObject)`, check if `food_item == null`. If null, `push_error` a descriptive message and set `_sim_hunger_gain = 0.0`.

**Do NOT touch:** Any logic that doesn't relate to null safety.

**Acceptance criteria:** `make test-godot` passes. `make lint-style` passes.

---

### Seed C — Agent Execution Robustness

**Files:** `addons/GdPlanningAI/scripts/nodes/gdpai_agent.gd`

**Task:** Harden `_on_plan_ready` and `_execute_plan` against edge cases.

1. In `_on_plan_ready` (line 182), add a bounds check before indexing `goals[goal_index]`. If `goal_index` is out of bounds, `push_error` and set `_current_goal = null`.
2. In `_execute_plan`, document the post-action block as a guaranteed cleanup phase. `post_perform_action` is called for every action in the chain, regardless of whether the plan completed or was aborted. `Action.post_perform_action` docstring must be updated to state the cleanup contract: implementations must be safe to call even if `pre_perform_action` or `perform_action` returned `FAILURE` or were never invoked.

**Do NOT touch:** Planning submission logic, `GdPAIRustBridge`, or other files.

**Acceptance criteria:** `make test-godot` passes. `make lint-style` passes.

---

### Seed D — Documentation & Style Fixes

**Files:** `addons/GdPlanningAI/scripts/refcounteds/precondition_builtin.gd`, `addons/GdPlanningAI/scripts/resources/gdpai_agent_config.gd`

**Task:** Fix malformed documentation and add missing value constraints.

1. In `precondition_builtin.gd`, restructure the `Target` enum so each value has its own `##` doc comment on the preceding line (Godot 4.2 style). Do the same for the `Op` enum.
2. In `gdpai_agent_config.gd`, add doc comments above `max_recursion` and `iteration_budget` noting the scheduler-enforced minimums (1 and 100 respectively).

**Do NOT touch:** Any logic.

**Acceptance criteria:** `make lint-style` passes.

---

### Seed E — Simplify GoToAction Binding Flattening

**Files:** `addons/GdPlanningAI/scripts/refcounteds/goto_action.gd`

**Task:** Investigate and clean up the nested-array-flattening logic for blackboard bindings.

1. Examine `get_action_cost` and `simulate_effect` in `goto_action.gd`. The binding flattening code:
   ```gdscript
   while flattened.size() > 0 and flattened[0] is Array:
       flattened = flattened[0]
   ```
   was added defensively because bindings were sometimes double-wrapped during bridge serialization. Since the recent Rust bridge changes pre-inject bindings directly into the agent blackboard, verify whether double-wrapping still occurs.
2. If the flattening is no longer needed, remove it and use `agent_blackboard.get_property("at_target")` directly. If it IS still needed, add a comment explaining why.

**Do NOT touch:** The Rust scheduler or `GdPAIRustBridge`.

**Acceptance criteria:** `make test-godot` passes. `make lint-style` passes. A comment explains the binding shape if flattening is retained.
