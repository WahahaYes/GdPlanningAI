# Research: Remove Dynamic Argument Dispatch from Scheduler Callbacks

**Date:** 2026-07-04  
**Scope:** `scheduler.rs` callback dispatch, Action template, bridge docs  
**Status:** Research complete — ready for implementation

---

## 1. Current Behaviour

The Rust scheduler's `dispatch_callback` function (`scheduler.rs:603`) inspects the GDScript callable's declared parameter count via `callable.get_argument_count()` and conditionally appends `provisions` (3rd arg) and `bindings` (4th arg) before invoking it.

This pattern is repeated identically for all three callback kinds:

- `CallbackKind::GetCost` (line 619)
- `CallbackKind::ApplyEffect` (line 651)
- `CallbackKind::EvalCustomPrecond` (line 679)

```rust
let mut args = vec![bb_agent.to_variant(), bb_world.to_variant()];
let expected_count = callable.get_argument_count();
if expected_count >= 3 {
    args.push(prov_arr.to_variant());
}
if expected_count >= 4 {
    args.push(bind_dict.to_variant());
}
callable.call(&args);
```

The `provisions_to_array` and `bindings_to_dict` helper functions exist solely to construct these 3rd/4th arguments.

---

## 2. Usage Audit

### 2.1 Example Actions (all 9 real-world actions)

| File | `get_action_cost` params | `simulate_effect` params |
|---|---|---|
| `eat_held_food_action.gd` | 2 | 2 |
| `wander_action.gd` | 2 | 2 |
| `pickup_action.gd` | 2 | 2 |
| `drop_item_action.gd` | — | — |
| `shake_tree_action.gd` | 2 | 2 |
| `cook_potato_action.gd` | 2 | 2 |
| `add_fuel_action.gd` | 2 | 2 |
| `dig_potato_action.gd` | 2 | 2 |
| `pick_up_wood_action.gd` | 2 | 2 |
| `goto_action.gd` | 2 | 2 |

**Result:** None use 3 or 4 parameters.

### 2.2 Custom Precondition Callables

Every `Precondition.custom(...)` callable in the examples uses 2 params, e.g.:

```gdscript
# shake_tree_action.gd
var tree_not_on_cooldown = func(_bb: GdPAIBlackboard, _ws: GdPAIBlackboard) -> bool:
    ...
```

### 2.3 Integration Tests

All integration test callables in:
- `test/integration/test_async_planner.gd`
- `test/integration/test_requirements_provisions.gd`
- `test/integration/test_blackboard_clone.gd`

use exactly 2 params for cost/effect callables.

### 2.4 Rust Unit Tests

Rust unit tests in `addons/GdPlanningAI/rust/tests/` do **not** exercise the GDScript callable path at all. They test planner logic directly against `ActionSpec` structs.

### 2.5 Templates

The `Action` template (both before and after the Seed A fix) documents only the 2-param form.

---

## 3. Why the Feature Is Redundant

Even if a user wanted provisions or bindings in their callback, they already receive them **via `inject_bindings_into_agent`** (`scheduler.rs:693`), which runs **before every callback** and writes bindings directly into the agent blackboard:

```rust
fn inject_bindings_into_agent(
    agent_bb: &mut Gd<GdPAIBlackboard>,
    bindings: &[(String, Vec<VariantSnapshot>)],
) {
    let mut bind = agent_bb.bind_mut();
    for (name, values) in bindings {
        if !values.is_empty() {
            bind.properties.insert(name.clone(), values[0].to_variant());
        }
    }
}
```

This means a callback can read any binding via `agent_blackboard.get_property("binding_name")` — the same mechanism used by `GoToAction` to resolve `at_target`.

Provisions are also available: an action's own provisions can be read from its `self` instance during planning (for object actions), or they are simply the action's own declared provisions which the action code already knows.

**In short:** There is no capability gained by passing `provisions` and `bindings` as extra arguments that isn't already available through the injected blackboard.

---

## 4. Proposed Simplification

### 4.1 Remove dynamic dispatch

In `scheduler.rs`, change all three callback arms to pass exactly 2 arguments:

```rust
// Before
let mut args = vec![bb_agent.to_variant(), bb_world.to_variant()];
let expected_count = callable.get_argument_count();
if expected_count >= 3 { ... }
if expected_count >= 4 { ... }

// After
let args = vec![bb_agent.to_variant(), bb_world.to_variant()];
```

### 4.2 Remove dead helper code

Delete:
- `provisions_to_array` (`scheduler.rs:705`)
- `bindings_to_dict` (`scheduler.rs:737`)

Delete unused locals in each callback arm:
- `prov_arr`
- `bind_dict`

### 4.3 Clean up `CallbackKind` in `plan_types.rs`

Remove `provisions` and `bindings` fields from all three `CallbackKind` variants. The scheduler only needs to pass agent + world snapshots. Move provision/binding data into the blackboard injection step only.

**Note:** `CallbackKind` currently carries `provisions: Vec<ProvisionSpec>` and `bindings: Vec<(String, Vec<VariantSnapshot>)>` purely to be converted into arrays/dicts for the 3rd/4th args. If we remove those args, we can also strip these fields from the enum variants.

However, `inject_bindings_into_agent` still needs the `bindings`. So we need to decide:
- **Option A:** Keep `bindings` in `CallbackKind` but only for `inject_bindings_into_agent`, not for GDScript args.
- **Option B:** Inject bindings into the agent snapshot *before* it crosses the callback boundary, so `CallbackKind` only carries the snapshots.

Option A is the minimal change. Option B is cleaner but touches the call sites in `planner/simulation.rs`.

### 4.4 Update Rust doc comments

Update the `CallbackKind` doc comments in `plan_types.rs:267` from:

```rust
/// Call `cost_callable(agent, world, provisions, bindings)` → return `Float(f64)`.
```

To:

```rust
/// Call `cost_callable(agent, world)` → return `Float(f64)`.
/// Bindings are pre-injected into `agent` before the call.
```

### 4.5 Update Action template comments

Remove the "adaptive arguments" comments from `get_action_cost` and `simulate_effect` in `script_templates/Action/template.gd` (already documented in Seed A fix, but now we know they describe a non-existent feature).

Replace with simple 2-param documentation.

### 4.6 Update `BRIDGE_AUDIT.md`

Mark items 5 and the `simulate_effect` adaptive argument note as addressed / N/A.

---

## 5. Files to Touch

| File | Change |
|---|---|
| `addons/GdPlanningAI/rust/src/scheduler.rs` | Remove `get_argument_count` checks; remove `provisions_to_array` and `bindings_to_dict`; delete unused locals |
| `addons/GdPlanningAI/rust/src/plan_types.rs` | Strip `provisions` and `bindings` from `CallbackKind` variants (or keep for injection only — see Option A vs B) |
| `addons/GdPlanningAI/rust/src/planner/simulation.rs` | Update call sites constructing `CallbackKind` if Option B chosen |
| `script_templates/Action/template.gd` | Simplify doc comments for `get_action_cost` and `simulate_effect` |
| `notes/BRIDGE_AUDIT.md` | Mark related items resolved |

---

## 6. Benefits

1. **Less code:** Removes ~60 lines of Rust (dispatch logic + helpers) and two helper functions.
2. **Less complexity:** No conditional argument building, no `get_argument_count` introspection.
3. **Clearer contract:** The API is simply `(agent_blackboard, world_state)` — always.
4. **Less confusion:** Users won't wonder why some examples have 2 params and others have 4.
5. **No functional loss:** All existing code continues to work unchanged.

---

## 7. Risks

- If any external user project relies on the 3/4 param form, this is a breaking change. However, the feature is undocumented in the template and no example uses it. The changelog should note this.
- `provisions_to_array` and `bindings_to_dict` might have been intended for future use. But the existing `inject_bindings_into_agent` mechanism covers the same need more cleanly.

---

## 8. Recommendation

**Proceed with the simplification.** The dynamic argument dispatch is an unused abstraction that adds complexity without value. The blackboard injection path already provides equivalent functionality.
