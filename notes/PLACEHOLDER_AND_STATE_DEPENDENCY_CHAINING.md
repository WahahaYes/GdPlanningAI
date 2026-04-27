# Placeholder Values and State Dependency Chaining

## Problem Statement

The GOAP planner works by chaining actions backward through preconditions, but simulates effects forward. This creates a fundamental mismatch when actions depend on state set by previous actions:

**Planning order (backward chaining):**
1. Goal: Reduce hunger
2. Action: Eat Held Food (reduces hunger)
3. Precondition: Must be holding food
4. Action: Pickup Food (sets held_item)

**Simulation order (forward):**
1. Pickup Food (sets held_item)
2. Eat Held Food (consumes held_item, reduces hunger)

When "Eat Held Food" is simulated during planning (step 2 of backward chaining), the agent doesn't have a `held_item` yet because the pickup action hasn't been simulated forward. This causes the simulation to fail or require placeholder values.

## Current Workaround

The current implementation uses placeholder values in `simulate_effect`:

```gdscript
# In eat_held_food_action.gd
if held_item_id.is_empty() or not hunger_restored_by_item.has(held_item_id):
    hunger_restored = 5.0  # Placeholder for planning
else:
    hunger_restored = float(hunger_restored_by_item[held_item_id])
```

And makes the goal threshold stricter to force chaining:

```gdscript
# In hunger_goal.gd
var required_hunger: float = max(0.0, current_hunger - 15.0)
```

This forces the planner to chain pickup + eat because:
- Eat alone: 30 → 25 (placeholder 5.0), goal requires < 15 → fails
- Pickup + Eat: 30 → 5 (20 from pickup + 5 from eat), goal requires < 15 → succeeds

## Limitations of Current Approach

1. **Hardcoded thresholds**: The -15.0 threshold is arbitrary and doesn't generalize
2. **Placeholder values are guesses**: The 5.0 placeholder doesn't reflect actual food values
3. **Tight coupling**: Goal threshold must be tuned to match placeholder values
4. **Doesn't scale**: Doesn't work well for actions with multiple dependencies
5. **Maintenance burden**: Changing food values requires adjusting thresholds

## Potential Solutions

### Solution 1: Backpropagation of State Requirements

**Concept:** When simulating an action with missing dependencies, track what state would be required and propagate that requirement backward through the planning chain.

**Implementation:**
- Modify `simulate_effect` to return a "state requirement" when dependencies are missing
- Example: "Eat Held Food" returns requirement: `held_item must be in hunger_restored_by_item`
- Planner searches for actions that satisfy this requirement (pickup actions)
- When pickup is found, the planner can simulate with the actual value

**Pros:**
- No hardcoded placeholder values
- Automatically discovers correct values through backpropagation
- Generalizes to any state dependency

**Cons:**
- Significant changes to planner architecture
- More complex planning algorithm
- May increase planning time

### Solution 2: Deferred Simulation with State Proxies

**Concept:** During backward chaining, create "state proxies" that represent what the state *would be* if preconditions were satisfied. Use these proxies for simulation.

**Implementation:**
- When evaluating "Eat Held Food" without held_item, create a proxy state with `held_item = "any_valid_food"`
- Simulate with proxy to estimate effect
- Track which specific food item was chosen
- When planning pickup, ensure it provides that specific item

**Pros:**
- More accurate than arbitrary placeholders
- Maintains forward simulation model
- Can be implemented in GDScript layer

**Cons:**
- Still requires some estimation
- Proxy management adds complexity
- Need to handle "any_valid_food" resolution

### Solution 3: Two-Phase Planning

**Concept:** Split planning into two phases:
1. **Structure phase**: Plan the action chain without simulating effects (just check preconditions)
2. **Simulation phase**: Once chain is determined, simulate forward with actual values

**Implementation:**
- Phase 1: Goal → Eat (needs held_item) → Pickup (provides held_item)
- Phase 2: Simulate Pickup (sets held_item="banana") → Simulate Eat (uses banana's value)
- If simulation fails or doesn't satisfy goal, backtrack to Phase 1

**Pros:**
- Clean separation of concerns
- No placeholder values needed
- Can use actual values from simulation

**Cons:**
- Two planning passes increases computation
- May need multiple iterations if simulation fails
- More complex state management

### Solution 4: Action Composition / Composite Actions

**Concept:** Create composite actions that represent common patterns (e.g., "Pickup and Eat Food").

**Implementation:**
- Define `PickupAndEatFoodAction` that internally chains pickup → eat
- This action has its own `simulate_effect` that correctly models both steps
- Planner treats it as a single atomic action

**Pros:**
- Simple to implement
- No changes to planner needed
- Accurate simulation

**Cons:**
- Loses flexibility (can't reuse individual actions)
- Need to predefine all common patterns
- Doesn't solve the general case

### Solution 5: Lazy Simulation with Memoization

**Concept:** Delay simulation until all dependencies are satisfied, cache results, and reuse them.

**Implementation:**
- When "Eat Held Food" is simulated without held_item, mark it as "pending"
- Continue backward chaining to find pickup
- Once pickup is found, simulate it and cache the resulting held_item
- Re-simulate "Eat Held Food" with the cached held_item

**Pros:**
- No placeholder values
- Uses actual values from dependency chain
- Can memoize for efficiency

**Cons:**
- Requires tracking pending simulations
- More complex state management
- May need multiple passes

### Solution 6: Symbolic Execution

**Concept:** Use symbolic values instead of concrete values during planning. Track constraints and resolve them at the end.

**Implementation:**
- Simulate with symbolic values: `hunger - hunger_restored_by_item[held_item]`
- Track constraint: `held_item must be in hunger_restored_by_item`
- When pickup is planned, resolve: `held_item = "banana"`, `hunger_restored = 20.0`
- Substitute and check if goal is satisfied

**Pros:**
- Most accurate representation
- No guessing or placeholders
- General solution

**Cons:**
- Very complex to implement
- Requires symbolic math library
- Significant architecture changes

## Recommendation

**Short-term:** Continue with current workaround (placeholder + stricter threshold) but:
- Document the pattern clearly
- Make placeholder values configurable per action
- Add validation to ensure thresholds are reasonable

**Medium-term:** Implement **Solution 2 (Deferred Simulation with State Proxies)** as it:
- Balances accuracy and complexity
- Can be implemented in GDScript layer
- Doesn't require Rust planner changes
- Generalizes to other use cases

**Long-term:** Consider **Solution 1 (Backpropagation)** or **Solution 6 (Symbolic Execution)** for a more fundamental solution, but this would require significant architecture work.

## Open Questions

1. How common is this pattern? Are there other actions with similar state dependencies?
2. What's the performance impact of two-phase planning?
3. Can we detect when placeholder-based planning is likely to fail?
4. Should we provide a DSL for defining action chains explicitly?
