# Campfire Test Debugging — Findings & Suspected Root Cause

## Fixed
- **`get_groups` → `get_group_labels`** in `sim_object_proxy.rs`: Rust called wrong GDScript method. Fixed, rebuilt. `get_proxies_in_group("CampfireObject")` now works.

## Verified Working
- `MaintainFireGoal.compute_reward()` correctly finds 1 campfire.
- `EatHeldFoodAction.simulate_effect()` correctly reduces hunger when `held_item = "cooked_potato"`.
- Planner identifies `EatHeldFoodAction` as candidate at depth 0 via `potential_bound_effect_satisfied_precondition_indices`.

## Suspected Root Cause: `GoToAction` cost heuristic too high

The planner trace shows it going down a wrong path instead of selecting `GoToAction`:

```
Depth 0: Eat Held Food (cost 1.00)
Depth 1: Cook Potato (cost 2.00)
Depth 2: Dig Potato (cost 0.70)
Depth 3: Add Fuel (cost 1.00)        ← should be GoToAction!
Depth 4: Pick Up Wood (cost 0.50)
Depth 5: Add Fuel (cost 1.00)
...spirals to depth 10 (max_recursion)...
```

At depth 3, open requirements are `at_target` (campfire) + `at_target` (potato). `GoToAction` provides `fact_wildcard("at_target")` which satisfies both. But its cost heuristic is **10.0** (see `goto_action.gd:76`), while `AddFuelAction` costs **1.0**. The planner tries cheaper candidates first.

**Why `AddFuelAction` is a candidate at depth 3**: `AddFuelAction` requires `at_target` (campfire). The `potential_bound_effect_satisfied_precondition_indices` function collects ALL provisions from ALL actions — including `GoToAction`'s `fact_wildcard("at_target")`. So `AddFuelAction`'s requirement CAN be hypothetically satisfied, making it a valid candidate. But selecting it opens new requirements (`held_item = "wood"`) leading to the wood/fuel spiral.

## Possible Fixes

### Option A: Lower `GoToAction`'s heuristic cost
In `goto_action.gd:76`, change `return 10.0` to `return 0.1` or `return 0.0`. This makes `GoToAction` always cheaper than other actions, so the planner tries it first.

### Option B: Filter irrelevant candidates
In the planner, when checking if an action's provisions satisfy open requirements (section 1 of `action_candidates_for_needs`), only consider actions whose provisions ACTUALLY match. `AddFuelAction`'s provision `held_item = ""` doesn't match `at_target` requirements, so it shouldn't be a candidate via section 1. But it becomes a candidate via section 2 (`potential_bound_effect_satisfied_precondition_indices`) because its own `at_target` requirement can be hypothetically satisfied by `GoToAction`'s wildcard. The `potential_bound_effect_satisfied_precondition_indices` path should only apply when the action's effect satisfies an open PRECONDITION, not when it just has unsatisfied requirements.

### Option C: Ensure `GoToAction` validity checks pass
`GoToAction.get_validity_checks()` includes `agent_has_object_data_of_group("GdPAILocationData")` which calls `bb.get_proxies_in_group("GdPAILocationData")` on the agent blackboard. Verify the agent blackboard snapshot correctly preserves `GdPAILocationData` objects with proper group labels.

## Next Step
Try **Option A** first — it's the simplest and most likely fix. Change `goto_action.gd:76` from `return 10.0` to `return 0.1`.
