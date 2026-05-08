# Hunger Example Planner Limitation

## Issue
The integration test `test_hunger_example_smoke.gd` expects the agent to plan "Pickup" after food is dropped, but the planner chooses "Wander" instead.

## Root Cause (Updated)
The planner uses `potential_bound_effect_satisfied_precondition_indices` which adds ALL possible provisions from ALL actions when evaluating if an action can satisfy preconditions. This causes Eat to be evaluated with `held_item="banana"` even before Pickup is added to the chain, making the planner think Eat can reduce hunger when it actually can't without Pickup.

### Evidence from Logs
From test_output10.txt:
- Line 363: `[EatHeldFoodAction] simulate_effect - hunger: 30.0, held_item: banana` at depth 0
- This happens BEFORE Pickup is added to the chain
- The held_item="banana" comes from Pickup's provision, but Pickup hasn't been selected yet
- Line 367: "Action 'Eat Held Food' can satisfy a need" - selected because it thinks held_item is available
- Line 401-446: Planner gets stuck in a loop trying to extend the chain because Eat's precondition isn't actually satisfied

The `potential_bound_effect_satisfied_precondition_indices` function (planner.rs lines 462-483) adds ALL provisions from ALL actions to the potential_provisions list:
```rust
for provider in ctx.actions {
    for provision in &provider.provisions {
        if !potential_provisions.contains(provision) {
            potential_provisions.push(provision.clone());
        }
    }
}
```

This means when evaluating Eat at depth 0, it uses Pickup's held_item provision even though Pickup hasn't been added to the chain. Eat's simulate_effect then reduces hunger (line 364: "hunger_restored: 20.0, new hunger: 10.0"), making it appear to satisfy the hunger goal. But when the chain is actually built, the held_item provision isn't bound, so the chain doesn't work correctly.

## Real Bug Fixes Made
1. **Rust snapshot.rs**: Fixed `into_blackboard()` to populate GDPAI_OBJECTS property when converting snapshot back to GDScript. Previously, world objects were stored in a separate HashMap and not accessible to GDScript simulate_effect methods.

2. **GdPAIObjectData**: Added `get_groups()` method to return actual group membership. This is needed for SimObjectProxy to capture group membership for planning, allowing actions to check group membership during simulation.

3. **FruitTreeObject**: Fixed `drop_fruit()` to add dropped fruit to tree's owner node instead of `get_tree().root`. This ensures the world node can discover dropped FoodObjects via `get_tree().get_nodes_in_group("GdPAIObjectData")`.

## Attempted Fixes (Reverted)
1. **Conditional hunger reduction in ShakeTreeAction**: Tried to only simulate hunger reduction if no FoodObject exists on ground. This didn't fix the issue because the planner still couldn't recognize the Pickup → Eat chain as complete.

2. **Removing hunger > 0 precondition from EatHeldFoodAction**: Tried to simplify the action's preconditions. This didn't help because the fundamental issue is the planner's provision handling, not the precondition complexity.

3. **Simulated state tracking in planner**: Added simulated_agent and simulated_world fields to PlanBranch to track state changes as actions are added. This would fix the issue but broke existing requirements/provisions tests because it changed fundamental planner behavior. Reverted due to regressions.

4. **Removing potential_bound_effect_satisfied_precondition_indices**: Removed the function to prevent using unbound provisions. Test results:
   - Smoke test still failed (Wander instead of Pickup)
   - Broke 5 requirements/provisions tests (test_pickup_eat_chain_satisfies_hunger, test_action_order_is_pickup_then_eat_not_reversed, test_requirement_dependent_effect_uses_provider_bound_resimulation, test_search_returns_cheapest_valid_requirement_chain, test_binding_in_set_requires_world_group_membership)
   - All returned empty action chains instead of expected Pickup → Eat chains
   - Reverted due to regressions.

## Why Existing Tests Work
The existing requirements/provisions tests (`test_requirements_provisions.gd`) rely on `potential_bound_effect_satisfied_precondition_indices` to evaluate actions with potential provisions from unselected predecessor actions. This allows them to find valid chains where provisions are provided by actions later in the chain.

The hunger example exposes a limitation because:
- Eat's simulate_effect requires held_item to be set to reduce hunger
- The planner evaluates Eat with Pickup's provision (held_item="banana") even before Pickup is selected
- This makes Eat appear to satisfy the hunger goal when it actually can't without Pickup
- The planner then gets stuck trying to build a chain that doesn't actually work

## Proposed Fix
Modify `potential_bound_effect_satisfied_precondition_indices` to only include provisions from actions whose preconditions are satisfied by the initial state. This prevents using provisions from actions that can't actually execute as predecessors.

Current behavior (lines 468-475):
```rust
for provider in ctx.actions {
    for provision in &provider.provisions {
        if !potential_provisions.contains(provision) {
            potential_provisions.push(provision.clone());
        }
    }
}
```

Proposed fix:
```rust
for provider in ctx.actions {
    // Only include provisions from actions whose preconditions are satisfied by initial state
    let preconditions_satisfied = provider.preconditions.iter().all(|p| {
        p.evaluate_builtin(ctx.initial_agent, ctx.initial_world).unwrap_or(false)
    });
    
    if preconditions_satisfied {
        for provision in &provider.provisions {
            if !potential_provisions.contains(provision) {
                potential_provisions.push(provision.clone());
            }
        }
    }
}
```

For the hunger example:
- PickupAction has a precondition `held_item == null or held_item == ""` (line 44-46 in pickup_action.gd)
- In the initial state, held_item is empty, so Pickup's precondition is satisfied
- Pickup's provision (held_item="banana") would be included in potential_provisions
- Eat would still be evaluated with held_item="banana"

Wait, this won't fix the issue because Pickup's precondition IS satisfied in the initial state (held_item is empty). The problem is that Pickup is being considered as a potential predecessor for Eat even though it's not in the chain yet.

A better fix: Only include provisions from actions that are already in the action_chain. This ensures that only actually selected predecessor actions can provide provisions for evaluation.

## Test Assumptions
The test `test_hunger_example_smoke.gd` assumes:
1. The planner can find the Pickup → Eat chain within max_depth=4
2. The planner will recognize when a goal is satisfied by the action chain
3. The planner won't get stuck trying to extend the chain beyond what's needed

These assumptions don't match the current planner's limitations due to the potential provision evaluation issue.

## Potential Solutions
1. **Fix potential provision evaluation**: Only use provisions from actions already in the action_chain, not from all potential actions. This is the targeted fix for the specific issue.

2. **Fix the planner to track simulated state**: Implement proper simulated state tracking during backward chaining, checking preconditions against the state before each action executes. This is the comprehensive fix but requires significant planner refactoring and may break existing tests.

3. **Increase max_recursion**: Won't help because the planner will just loop deeper, not recognize completion.

4. **Modify the test expectations**: Mark the test as expected failure until planner is improved, or change the test to work within current limitations.

## Recommendation
The simple fix of removing `potential_bound_effect_satisfied_precondition_indices` broke existing tests and didn't fix the smoke test. We need a more targeted approach.

Potential targeted fixes to investigate:
1. **Check if provisions are from spatially valid actions**: Only include provisions from actions that are spatially valid (e.g., PickupAction requires the agent to be near the target). This would prevent using Pickup's provision if the agent isn't near the banana.

2. **Check if provisions have satisfied preconditions**: Only include provisions from actions whose preconditions can be satisfied by the current state. For PickupAction, the precondition `held_item == null or held_item == ""` is satisfied in the initial state, so this won't help.

3. **Track which actions are spatially valid in the current branch**: Add spatial validity checking to the branch state and only use provisions from spatially valid actions.

4. **Make EatHeldFoodAction's simulate_effect work without held_item during planning**: Modify the action to simulate hunger reduction even without a valid held_item, with a default value. This is a workaround but doesn't address the root cause.

5. **Document as known limitation**: Defer the fix to future work and update the test expectations to match current behavior.

The most promising approach is #1 or #3 - adding spatial validity awareness to the provision evaluation.
