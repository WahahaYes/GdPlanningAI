# Issue Recap: GDScript Parse/Runtime Errors in Examples

**Date:** 2026-06-04
**Status:** Investigated, Fix Pending

## Problem Summary
Recent changes to the `Precondition.custom` API and the reorganization of examples have left the codebase in a partially broken state. Specifically, custom interaction actions in the campfire example are using an outdated signature for `Precondition.custom()`, and some class name conflicts or script compilation failures are preventing GUT tests from running correctly.

## Root Causes

1. **API Mismatch in `Precondition.custom()`**:
   - **Current API**: `static func custom(fn: Callable) -> Precondition` (from `addons/GdPlanningAI/scripts/refcounteds/precondition.gd`).
   - **Broken Usage**: `preconds.append(Precondition.custom(self , "_is_holding_food"))` in `eat_held_food_action.gd`.
   - **Error**: `Too many arguments for "custom()" call. Expected at most 1 but received 2.`

2. **Hunger Behavior Config Compilation Failure**:
   - `HungerBehaviorConfig` fails to compile, likely due to a nonexistent function `new` call on a script that failed to load (possibly `EatHeldFoodAction`).
   - This cascades into `test_campfire_example_smoke.gd` failing during setup.

3. **GoToAction Pattern Migration State**:
   - The project is in the middle of migrating from `SpatialAction` to a decoupled `GoToAction` + `InteractionAction` pattern.
   - `EatHeldFoodAction` (agent-provided) and various object-provided actions (Dig, Cook, Add Fuel) are using the new `at_target` requirement pattern, but their GDScript implementations need validation against the current framework.

## Potential Fixes

1. **Fix `Precondition.custom` calls**:
   - Update all occurrences of `Precondition.custom(obj, method_name)` to `Precondition.custom(Callable(obj, method_name))`.
   - Impacted files: `examples/behaviors/campfire/eat_held_food_action.gd`, and potentially others in `examples/objects/`.

2. **Verify/Update `HungerBehaviorConfig`**:
   - Fix the broken `EatHeldFoodAction.new(...)` chain in `hunger_behavior_config.gd`.
   - Ensure `EatHeldFoodAction` is correctly loaded/referenced.

3. **Validate GoToAction Chaining**:
   - Ensure all interaction actions correctly return `RequirementSpec.fact("at_target", [location_data])`.
   - Verify `GoToAction` is included in all relevant behavior configs.

## Next Steps
1. Apply fix to `Precondition.custom` in `eat_held_food_action.gd`.
2. Check `examples/behaviors/hunger/hunger_behavior_config.gd` for similar issues or missing `class_name` references.
3. Re-run `make test-godot` to verify the fix.
