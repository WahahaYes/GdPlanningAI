# Retire plan.gd — Legacy Planner Removal Plan

## Objective

Remove the legacy GDScript planner (`plan.gd`), wire `GdPAIAgent` exclusively to the
Rust bridge, and clean up all the dead code that only existed to support the old planner.

## Why It's Already Safe

`plan.gd` calls `_agent.blackboard.copy_for_simulation()` but `GdPAIBlackboard` is now a
Rust GDExtension class whose `clone_for_simulation()` is a private Rust method — **not a
`#[func]`** — so the old planner path is already non-functional at runtime. Removing it is
a confirmed-dead cleanup, not a risk.

---

## Current State (branch: rust_migration)

- `GdPAIRustBridge` exists in `scripts/gdpai_rust_bridge.gd` and is fully functional
- `RustPlanningEngine` (Rust GDExtension) is built and tested via `simple_bridge_test`
- `GdPAIAgent` still instantiates `Plan` objects — this needs to be replaced
- The test at `examples/simple_bridge_test/simple_bridge_test.tscn` passes and should
  continue to pass after each phase

---

## Files to DELETE

| File | Reason |
|------|--------|
| `addons/GdPlanningAI/scripts/refcounteds/plan.gd` | The entire legacy planner |

---

## Phase 1 — Remove `is_satisfied` and `copy_for_simulation` from Precondition

These exist only to serve `plan.gd`'s mutable-state branching model.
The Rust bridge snapshots `is_satisfied` at planning start and never mutates it.

### `addons/GdPlanningAI/scripts/refcounteds/precondition.gd`

Remove `var is_satisfied: bool = false`.

Simplify `evaluate()` to a direct delegation (no more caching):
```gdscript
func evaluate(agent: GdPAIBlackboard, world: GdPAIBlackboard) -> bool:
    return _do_evaluate(agent, world)
```

Remove `copy_for_simulation()` method entirely.

### `addons/GdPlanningAI/scripts/refcounteds/precondition_builtin.gd`

Remove entire `copy_for_simulation()` override (lines ~37-40).

### `addons/GdPlanningAI/scripts/refcounteds/precondition_custom.gd`

Remove entire `copy_for_simulation()` override (lines ~12-15).

### `addons/GdPlanningAI/scripts/gdpai_rust_bridge.gd`

In `_extract_preconditions()`, remove `"is_satisfied": precond.is_satisfied` from both
the builtin dict and the custom callback dict. The Rust side no longer needs it.

### `addons/GdPlanningAI/rust/src/precondition.rs`

- Remove `pub is_satisfied: bool` from `PreconditionHandler` struct
- Remove `is_satisfied` parsing in `from_dict()` (the `dict.get("is_satisfied")...` block)
- Remove the early-return short-circuit at the top of `evaluate()`:
  ```rust
  // REMOVE THIS:
  if self.is_satisfied {
      return true;
  }
  ```

After this phase: rebuild Rust (`make build-debug`), run `simple_bridge_test` — should pass.

---

## Phase 2 — Delete `plan.gd` and remove its references

Delete `addons/GdPlanningAI/scripts/refcounteds/plan.gd`.

Then grep for `Plan` usage and remove all references:
```
grep -r "Plan\." addons/GdPlanningAI --include="*.gd"
grep -r ": Plan" addons/GdPlanningAI --include="*.gd"
grep -r "Plan.new()" addons/GdPlanningAI --include="*.gd"
```

Expected remaining references are only in `gdpai_agent.gd` (handled in Phase 3).

---

## Phase 3 — Rewrite `gdpai_agent.gd` to use the Rust bridge

This is the largest change. The agent currently:
1. Creates `Plan` objects for each goal in `_select_highest_reward_goal()`
2. Stores the winning plan as `_current_plan: Plan`
3. Extracts actions via `_current_plan.get_plan()`
4. Uses `_current_plan.get_plan_tree_debug_data()` for the debugger

After migration, the agent will:
1. Call `_bridge.build_plan(self, all_actions, goals)` — Rust selects the best goal
2. Store the result as `_current_action_chain: Array[Action]`
3. Execute `_current_action_chain` directly

### Variable changes

Remove:
```gdscript
var _current_plan: Plan = null
var _thread: Thread = null
```

Add:
```gdscript
var _bridge: GdPAIRustBridge
var _current_action_chain: Array[Action] = []
```

### `_ready()` changes

Remove:
```gdscript
if config.use_multithreading:
    _thread = Thread.new()
```

Add after the existing blackboard setup:
```gdscript
_bridge = GdPAIRustBridge.new()
```

### Remove these methods entirely

- `_select_highest_reward_goal()` — replaced by Rust bridge
- `_sync_multithreaded_plan()` — threading no longer needed

### Rewrite `_query_world_state_and_plan()`

```gdscript
func _query_world_state_and_plan() -> void:
    var worldly_actions: Array[Action] = await _compute_worldly_actions()
    var valid_self_actions: Array[Action] = await _compute_valid_self_actions()
    var all_actions: Array[Action] = []
    all_actions.append_array(valid_self_actions)
    all_actions.append_array(worldly_actions)

    var result: Dictionary = _bridge.build_plan(self, all_actions, goals)
    _current_plan_step = -1
    if result.get("success", false):
        _current_action_chain = _bridge.deserialize_plan_result(result, all_actions)
        _reset_runtime_status(_current_action_chain)
    else:
        _current_action_chain = []
        _current_goal = null
    _update_debugger_info()
```

Note: `_current_goal` tracking is simplified here. The Rust result does not currently
return which goal was selected. This can be tracked later by adding a `goal_index` field
to the Rust result dict if needed for debugging.

### Update `_process()`

Replace:
```gdscript
if _current_plan == null or _current_plan_step > _current_plan.get_plan().size():
    await _query_world_state_and_plan()
```
With:
```gdscript
if _current_action_chain.is_empty() or _current_plan_step > _current_action_chain.size():
    await _query_world_state_and_plan()
```

### Update `_on_planning_timer_timeout()`

Replace:
```gdscript
if _current_plan == null or _current_plan_step > _current_plan.get_plan().size():
```
With:
```gdscript
if _current_action_chain.is_empty() or _current_plan_step > _current_action_chain.size():
```

### Update `_execute_plan()`

Replace:
```gdscript
if _current_plan == null:
    return
var action_chain: Array[Action] = _current_plan.get_plan()
```
With:
```gdscript
if _current_action_chain.is_empty():
    return
var action_chain: Array[Action] = _current_action_chain
```

### Update `get_current_plan()`

This method currently returns `Plan`. Change it to return the action chain:
```gdscript
## Returns the current action chain being executed.
func get_current_plan() -> Array[Action]:
    return _current_action_chain
```

### Update `_update_debugger_info()`

Remove the `get_plan_tree_debug_data()` call. Replace with flat action chain info:
```gdscript
func _update_debugger_info() -> void:
    var agent_info: Dictionary = {}
    if not _current_action_chain.is_empty():
        var actions_info: Array[Dictionary] = []
        for action in _current_action_chain:
            actions_info.append({
                "id": str(action.get_instance_id()),
                "action": action.get_title(),
                "action_description": action.get_description(),
            })
        agent_info["plan_tree"] = _inject_runtime_status({
            "id": "",
            "action": "Plan Root",
            "children": actions_info,
        })
    if _current_goal != null and is_instance_valid(_current_goal):
        agent_info["current_goal"] = _current_goal.get_title()
        agent_info["current_goal_description"] = _current_goal.get_description()
    EngineDebugger.send_message("gdplanningai:update_agent_info", [get_instance_id(), agent_info])
```

---

## Phase 4 — Clean up `gdpai_agent_config.gd`

Remove these three exported vars (they only configure the old planner):

```gdscript
@export var use_multithreading: bool = false
@export var thread_priority: Thread.Priority = Thread.PRIORITY_LOW
@export var max_recursion: int = 4
```

**Note:** Removing exported vars will show as missing in any saved `.tres` resources that
set these values. Check all `.tres` agent config files and delete those property entries.
Search: `grep -r "use_multithreading\|thread_priority\|max_recursion" --include="*.tres"`.

---

## Phase 5 — Remove `copy_for_simulation` from `GdPAIObjectData`

`GdPAIObjectData.copy_for_simulation()` was used by the old planner to clone live objects
for simulation. The Rust bridge now uses `get_sim_properties()` instead, making this dead.

### `script_templates/GdPAIObjectData/template.gd`

Remove the entire `copy_for_simulation()` block (the `# Override` comment through the
closing `return new_data` line). Also remove the `assign_uid_and_entity()` call within it —
that helper no longer exists on `GdPAIObjectData`.

### `addons/GdPlanningAI/examples/fruit_tree/sample_fruit_tree_object.gd`

Remove the entire `copy_for_simulation()` override (lines ~86-91 as of this writing).

### Scan for any other implementations

```
grep -r "copy_for_simulation" addons/ --include="*.gd"
```

Should return zero results after removing the above.

---

## Phase 6 — Remove `is_a_copy` assignments

`plan.gd` set `blackboard.is_a_copy = true` on cloned blackboards. Since `GdPAIBlackboard`
is a Rust class without an `is_a_copy` property, these calls are already no-ops (or errors).
After deleting `plan.gd`, do a final check:

```
grep -r "is_a_copy" addons/ --include="*.gd"
```

Should return zero results.

---

## Verification Steps (run after each phase)

1. After Phase 1+Rust rebuild: `godot --headless --path . res://addons/GdPlanningAI/examples/simple_bridge_test/simple_bridge_test.tscn`
   Expected: `[TEST PASSED] SUCCESS`

2. After Phase 3: Open editor and run the hunger example scene. Agent should plan and
   execute actions using the Rust bridge.

3. After all phases: Full search for any remaining references:
   ```
   grep -r "Plan\b" addons/GdPlanningAI --include="*.gd"
   grep -r "copy_for_simulation" . --include="*.gd"
   grep -r "is_satisfied" addons/GdPlanningAI --include="*.gd"
   grep -r "reverse_simulate_effect" . --include="*.gd"
   ```
   All should return zero or only doc comment references.

---

## Summary of All Touched Files

| File | Action |
|------|--------|
| `scripts/refcounteds/plan.gd` | **DELETE** |
| `scripts/nodes/gdpai_agent.gd` | Major rewrite — use Rust bridge |
| `scripts/resources/gdpai_agent_config.gd` | Remove 3 dead exports |
| `scripts/refcounteds/precondition.gd` | Remove `is_satisfied`, `copy_for_simulation`, simplify `evaluate()` |
| `scripts/refcounteds/precondition_builtin.gd` | Remove `copy_for_simulation()` |
| `scripts/refcounteds/precondition_custom.gd` | Remove `copy_for_simulation()` |
| `scripts/gdpai_rust_bridge.gd` | Remove `"is_satisfied"` from precondition dicts |
| `rust/src/precondition.rs` | Remove `is_satisfied` field and short-circuit |
| `script_templates/GdPAIObjectData/template.gd` | Remove `copy_for_simulation()` |
| `examples/fruit_tree/sample_fruit_tree_object.gd` | Remove `copy_for_simulation()` |
