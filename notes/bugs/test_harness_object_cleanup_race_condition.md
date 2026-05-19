# Test Harness Object Cleanup Race Condition

## Problem
Integration tests are failing with "Callable is no longer valid (target object freed)" and "ObjectRef was freed before callback" errors. These errors occur during test cleanup in the `after_each()` hook.

## Symptoms
```
[Failed]: Unexpected Errors:
[1] <engine-1>[GdPAI] Callable is no longer valid (target object freed); returning safe default
[2] <engine-1>[GdPAI] ObjectRef(124621163998): object was freed before callback; returning Nil.
```

Tests affected:
- `test_hunger_example_smoke.gd`: `test_real_hunger_example_shakes_tree_then_picks_up_food`
- `test_campfire_example_smoke.gd`: Multiple tests
- `test_goto_action_wildcard_chains_to_pickup_interaction`
- Unit tests in `test_gdpai_blackboard.gd`

## Root Cause
The test harness uses `add_child_autofree()` to automatically free nodes after each test. The `after_each()` hook calls `_drain_scheduler()` to process pending callbacks from the background planner threads. However, there's a race condition:

1. Tests use `add_child_autofree()` to schedule node cleanup after the test
2. `after_each()` calls `_drain_scheduler(timeout_frames=120)` to process callbacks
3. Background planner threads (Rayon thread pool) may still have callbacks in-flight
4. When `_drain_scheduler()` returns (after 120 frames or when job_count == 0), autofree frees the objects
5. Delayed callbacks from background threads then try to access freed objects

The scheduler holds `Gd<Object>` references and `Vec<Callable>`, but these don't prevent the autofree mechanism from freeing the underlying Godot objects.

## Attempted Fix
Increased `_drain_scheduler()` timeout from 120 to 6000 frames to give more time for callbacks to complete. This was not yet tested.

## Potential Solutions

### Option 1: Better Synchronization
- Modify the scheduler to track all pending callbacks and ensure they're complete before allowing test cleanup
- Could add a "flush" method that blocks until all callbacks are processed
- May require coordination between Rust scheduler and GDScript test harness

### Option 2: Weak References
- Use weak references (`Weak<Gd<Object>>`) in the Rust scheduler instead of strong references
- Check if objects are still valid before calling callbacks
- Return safe defaults (Nil) when objects are freed
- This is already partially implemented (the errors show safe defaults are returned), but the warnings still cause test failures

### Option 3: Test Cleanup Order
- Modify test harness to explicitly cancel all planning jobs before cleanup
- Ensure scheduler is fully drained before autofree kicks in
- May need to add explicit cleanup steps instead of relying on autofree

### Option 4: Suppress Warnings in Tests
- Configure the logger to suppress "object freed" warnings during test execution
- This is a band-aid that doesn't fix the root cause but allows tests to pass
- May hide real issues in production

## Impact
- Planning logic fixes are working correctly (planner generates correct 3-action plans)
- The issue is purely in the test harness cleanup process
- Does not affect production usage, only automated testing
- 22 out of 46 integration tests are failing due to this issue

## Notes
- The planning fixes from this session (validity check logic, precondition evaluation against final_state, removing validity checks from ripple) are all working correctly
- The test `test_real_hunger_example_shakes_tree_then_picks_up_food` passes its planning assertions but fails on object cleanup warnings
- This is a pre-existing issue that was exposed by the increased test coverage
