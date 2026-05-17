# Campfire Non-Convergence Investigation

**Status:** Testing refined A* implementation with rigorous forward simulation.

## Observed Bug
The agent in the Campfire example was failing to find valid plans for the **Hunger** goal, even after the modularization of search strategies. In interactive testing, the planner would either time out or return an empty plan despite the agent being hungry.

## Root Causes Discovered

1.  **A* Min-Heap Mismatch**: 
    Rust's `BinaryHeap` is a max-heap. My initial implementation of `SearchNode::cmp` for A* was returning `Ordering::Greater` for larger `f-scores`, meaning the planner was prioritizing the **most expensive** paths first. Fixed in `controller.rs`.

2.  **Custom Precondition Callback Overhead**:
    The `HungerGoal` used a custom `Callable` to check `hunger < threshold`. This required a cross-thread callback to GDScript for every node expansion. This is both slow and prone to synchronization issues. Fixed by switching to built-in `agent_property_leq_than` in `hunger_goal.gd`.

3.  **Backward/Forward Simulation Drift**:
    The planner builds chains backward (Suffix -> Prefix) but was attempting to track "accumulated state" during this backward construction. This is fundamentally flawed because the effect of a prefix action (like `Pickup`) changes the context in which a suffix action (like `Eat`) is evaluated. Fixed by implementing rigorous **forward re-simulation** in `expander.rs`.

4.  **Goal Satisfaction Verification**:
    The `is_complete` check was only verifying that the most recently added prefix action's needs were met. It wasn't checking if the *original* goal was still satisfied at the end of the full chain. Fixed by storing `goal_preconditions` in `PlanBranch`.

## Hypotheses & Solutions Being Tested

-   **Hypothesis**: The search space for `GoTo` is extremely broad (fan-out to every wood pile and potato). Without a strong `A*` and correct simulation, the planner gets lost in suboptimal "walking" chains.
-   **Solution**: Rigorous `simulate_chain` now validates every step of the chain forward from the initial state. If a precondition or requirement fails at any point during simulation (e.g., trying to cook before the wood is actually in the fire), the branch is pruned immediately.
-   **Hypothesis**: `max_recursion = 6` is too shallow for the Campfire sequence (Dig -> Move -> Pickup -> Move -> Cook -> Move -> Eat).
-   **Solution**: Increased default `max_recursion` to 10 in `campfire_agent_config.tres`.

## Current Status
Headless diagnostics (`campfire_diag_v6.log`) show the planner is now correctly identifying `Eat Held Food` as a candidate and branching into its requirements. Forward simulation is pruning invalid chains early. Re-running final validation.
