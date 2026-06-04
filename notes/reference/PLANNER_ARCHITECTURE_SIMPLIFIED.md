# Simplified Backward-Chaining Planner Architecture

## Core Philosophy
The planner is a **Regressive Search** (Backward Chaining) engine. It starts from a Goal and builds a timeline toward the Present. To keep the deep simulation stable, we maintain a **Snapshot Sequence** where every partial plan is grounded in the current reality.

## 1. The Data Model: Snapshot Sequence
Every `PlanBranch` stores:
- `action_chain`: `[A, B, C]` (where A is the first action to execute in the world).
- `agent_snapshots`: `[S0, S1, S2, S3]`
    - `S0`: Initial state (The "Right Now").
    - `S1`: State after action A.
    - `S2`: State after action B.
    - `S3`: State after action C (The "Accumulated Future").

## 2. The Search Step: Always Prepend
We avoid "middle insertion." All candidate actions are **prepended** to the front of the chain.

### Logic:
1.  **Candidate Discovery**: Look for actions that satisfy *any* open need (Precondition or Requirement) currently in the branch.
2.  **Initial Grounding**: Evaluate the candidate action against `S0` (Initial State).
    - Can I perform `Pickup` right now?
    - What is the cost of `GoTo` starting from where I am right now?
3.  **Expansion**:
    - **Prepend** the new action `P` to the chain: `[P, A, B, C]`.
    - **Insert** a new snapshot at index 1: `S1_new = P.simulate_effect(S0)`.
    - **Shift** existing snapshots: The old `S1` becomes the new `S2`, etc.
    - **Forward Ripple**: Re-simulate the suffix (`A, B, C`) starting from `S1_new` to update their effects and costs based on the new history.

## 3. Why This Works
- **Consistent Grounding**: The first action in the chain is always evaluated against the real world.
- **Deep Simulation**: By rippling the simulation forward after a prepend, `Eat` (at the end of the chain) eventually "sees" the food provided by `Pickup` (at the start of the chain).
- **Simplicity**: No complex logic to figure out where an action "belongs." If it helps reach the goal, it becomes the new "first step" of the plan.

## 4. Configuration: Ripple Policies
To balance performance and accuracy, agents can configure how often the forward ripple occurs:

- **`ALWAYS`**: Re-simulate the suffix on every prepend. Highest accuracy, lowest performance.
- **`ON_REQUIREMENT`**: Only re-simulate the suffix if the prepended action satisfies a `RequirementSpec` (symbolic dependency). 
- **`NEVER`**: Never re-simulate the suffix. Use only for simple planners where actions have no deep state dependencies.

## 5. Key Differences from Current Implementation
1.  **Remove `insertion_index_for_candidate`**: All insertions happen at `0`.
2.  **Remove `open_requirement_consumers`**: We don't need to track who needs what to place providers; the search naturally finds providers as it works backward from the needs.
3.  **Standardize Simulation**: `simulate_effect` is the source of truth for state; `Provisions/Requirements` are just hints for the search to find relevant actions.
