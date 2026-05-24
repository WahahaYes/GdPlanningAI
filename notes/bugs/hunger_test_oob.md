# Bug: Hunger Example Smoke Test - Out of Bounds Access

## Description
The test `test_real_hunger_example_shakes_tree_then_picks_up_food` in `test_hunger_example_smoke.gd` is failing with an "Out of bounds get index '2'" error. This indicates that the planner is returning a plan with fewer than the expected 3 actions (GoTo -> Shake -> Pickup -> Eat).

## Logs
```
SCRIPT ERROR: Out of bounds get index '2' (on base: 'Array[Action]')
   at: test_real_hunger_example_shakes_tree_then_picks_up_food (res://test/integration/test_hunger_example_smoke.gd:174)
```

## Initial Analysis
- The test expects a chain of at least 3 actions.
- The error happens at line 174 of the test, likely while asserting the contents of the plan array.
- This suggests the planner returned a plan of size 2 or less.
- Potential cause: The "optimistic restore" value in the `EatHeldFoodAction` (20.0) might be satisfying the hunger goal with a shorter plan than intended, similar to the "raw potato" issue found in the campfire tests.
- Alternatively, the `Shake Tree` action might not be correctly triggering the discovery of the `Pick Up` action, or the binding logic for the dropped fruit is failing.

## Tasks
- [x] Review `test_hunger_example_smoke.gd` around line 174 to see exactly what it's asserting.
- [x] Check the plan output in the debug logs for this specific test.
- [ ] Investigate if the "optimistic unbound restore" is causing a short-circuit.

## Update - May 23 (Execution Analysis)
We found that the **fruit is NOT dropping** because the **plan execution is failing at the first step**.

### Definitive Findings:
1.  **Wildcard Binding Failure**: Logs showed `[DEBUG] GoToAction failed: target_location is null`.
2.  **Root Cause**: The Rust `PlannerEngine` was not correctly handling `ProvisionSpec::FactWildcard`. When `GoToAction` provided a wildcard `at_target` to satisfy a requirement like `at_target(tree_location)`, the engine was satisfying the requirement but **failing to pass the arguments (`tree_location`) back to the action as a binding**.
3.  **Engine Logic Error**: In `engine.rs`, the loop that records satisfied requirements into `new_bindings` was only checking for `ProvisionSpec::Binding` and `ProvisionSpec::Fact`. It was completely ignoring `ProvisionSpec::FactWildcard`.
4.  **Result**: The `GoToAction` was being added to the plan with no `target_location`, causing it to fail immediately in `pre_perform_action` during real execution.

### Related Discoveries:
- **Optimistic Short-Circuit**: `ShakeTreeAction` has a `simulate_effect` that optimistically reduces hunger. This causes the planner to think the goal is satisfied after just shaking the tree, skipping the "Pick Up" and "Eat" actions. This needs to be removed or adjusted.
- **Group Registration Timing**: `GdPAIObjectData` was registering groups in `_init`. In some test scenarios, this might be too early for the `SceneTree` to correctly index them. Moving this to `NOTIFICATION_POST_ENTER_TREE` in `_notification` is safer.
- **Test Timeouts**: The exhaustive nature of Dijkstra search means some complex tests (like the campfire/hunger smoke tests) need higher timeouts (e.g. 600 frames) when running in a headless CI-like environment.

### Fix Implementation Details (to be re-applied):
In `addons/GdPlanningAI/rust/src/planner/engine.rs`, the `for (_, req, prov) in cand.satisfied_requirements` loop needs to handle the wildcard case:
```rust
ProvisionSpec::FactWildcard { fact_name } => {
    if let RequirementSpec::Fact { args, .. } = req {
        new_bindings.push((0, fact_name, args));
    }
}
```

### Next Steps:
- Re-apply the `FactWildcard` binding fix in `engine.rs`.
- Remove optimistic hunger reduction from `ShakeTreeAction.gd`.
- Ensure `GdPAIObjectData.gd` uses `_notification` for group registration.
- Verify the full 3-step plan: `Go To -> Pick Up -> Eat` (after tree is already shaken).
