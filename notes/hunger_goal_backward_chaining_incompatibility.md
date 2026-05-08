# Hunger Goal Backward Chaining Incompatibility

## Problem
The `test_real_hunger_example_shakes_tree_then_picks_up_food` test fails because the planner selects "Wander" instead of "Pickup" after food is dropped.

## Root Cause
The `PlanBranch` struct does not maintain a cumulative state snapshot as actions are added to the planning chain. The planner checks goal preconditions and action effects against the INITIAL state at each step of the backward search, rather than against the cumulative state after applying effects of actions in the chain.

### How Backward Chaining GOAP Works
1. Start from the goal (desired state)
2. Find actions whose effects satisfy the goal's preconditions
3. Add those actions as predecessors to the plan
4. The actions' preconditions become new open needs
5. Repeat until all open needs are satisfied by the initial state

### The Missing Cumulative State
Before the Rust migration, the planner cloned agent and blackboard states before simulating the effect of potential actions. This capability was lost during the migration. The planner should maintain an altered simulated state for each node as we build out the planning tree, rather than constantly checking against the INITIAL state.

The issue manifests when:
- The planner adds Eat Held Food to satisfy hunger < 15
- Eat Held Food requires held_item (via requirements)
- The planner adds Pick Up Item to provide held_item
- Pick Up Item doesn't satisfy the hunger precondition directly
- The planner checks if hunger < 15 is satisfied by the INITIAL state (it's not, hunger is 30)
- So it tries to add Eat Held Food again, creating a cycle

The planner should check if hunger < 15 is satisfied by the CUMULATIVE state after applying effects of the chain [Pickup -> Eat], which would be hunger = 10 (satisfied).

## Why the First Test Passes
The first hunger test (Shake Tree) passes because Shake Tree's effect directly satisfies the hunger precondition (reduces hunger from 30 to 10). The planner doesn't need to chain multiple actions, so the lack of cumulative state tracking doesn't cause a cycle.

## Why the Second Test Fails
The second hunger test (Pickup after food drops) fails because it requires chaining Pickup -> Eat. The planner gets stuck in a cycle because it can't recognize that the combined effect of the chain satisfies the goal precondition.

## Fix Implemented
Added cumulative state tracking to `PlanBranch` to maintain the state after applying effects of actions in the planning chain.

### Changes Made
1. **Added cumulative state fields to PlanBranch** (`planner.rs`):
   - `cumulative_agent: BlackboardSnapshot` - Cumulative agent state after applying effects
   - `cumulative_world: BlackboardSnapshot` - Cumulative world state after applying effects

2. **Updated PlanBranch::new()** to initialize cumulative state with initial state

3. **Updated is_complete()** to check preconditions against cumulative state instead of initial state

4. **Updated update_open_needs()** to apply action effects to cumulative state when adding actions to the chain

5. **Updated bound_effect_satisfied_precondition_indices()** to use cumulative state instead of initial state when checking if an action's effect satisfies preconditions

6. **Updated potential_bound_effect_satisfied_precondition_indices()** to use cumulative state

7. **Updated estimate_action_cost()** to use cumulative state for cost estimation

8. **Updated resolve_pending_effects()** to pass branch parameter for cumulative state access

### How the Fix Works
Now when the planner builds a chain:
- It starts with the initial state in cumulative_agent/cumulative_world
- When it adds an action to the chain, it applies the action's effect to the cumulative state
- When checking if the branch is complete, it checks preconditions against the cumulative state
- When finding candidate actions, it checks if an action's effect can satisfy preconditions given the cumulative state
- This allows the planner to recognize that [Pick Up Item, Eat Held Food] together satisfy the hunger goal (hunger < 15), even though neither action alone satisfies it when checked against the initial state

## Status
Cumulative state fix broke existing tests in `test_requirements_provisions.gd`. The baseline test_output3.txt shows that test_requirements_provisions.gd was passing before my changes (36/37 tests passing), but after my cumulative state changes, 6 tests in test_requirements_provisions.gd are now failing (31/37 tests passing).

This confirms that the existing tests were modeling correct behavior and my cumulative state approach broke the requirements/provisions chaining mechanism. Applying action effects to the cumulative state during backward chaining is incompatible with the planner's current design.

The root cause remains: the planner checks goal preconditions against the initial state at each step of the backward search, but the hunger precondition is only satisfied after the FULL chain executes (Pickup -> Eat), not by the initial state or by individual actions.

Need to find a different approach that doesn't break the requirements/provisions chaining mechanism.
