# Phase 2 Modular Search: Learnings & Retrospective

## Current Implementation Status
We attempted to move from a basic search to a modular architecture supporting:
- Multiple Search Strategies (DFS, A*, Dijkstra)
- Custom Termination Policies (First Valid, Budget-based)
- Goal Selection Strategies (Highest Reward, Best Ratio)
- Heuristic-guided A* search
- Closed Set (Visited Set) pruning

## Core Issues Identified

### 1. The Optimism vs. Reality Gap (The "Eat Wood" Problem)
Backward planning is naturally "optimistic"—it assumes an action satisfies a need until proven otherwise.
- **Bug**: `EatHeldFoodAction` optimistically "imagined" it restored hunger during the backward pass as long as *any* item was held.
- **Result**: The planner would find a 0-need branch for "Pick Up Wood -> Eat Wood". However, the **forward validation pass** (which uses the real simulation logic) would reject it because wood isn't in the food dictionary.
- **Learning**: Symbolic requirements (e.g., `binding_in_set("held_item", "Food")`) must be robust enough to prune impossible branches *before* they reach forward validation.

### 2. Closed Set Sensitivity (The "Float Variance" Problem)
We initially tried pruning states by hashing the entire `BlackboardSnapshot`.
- **Bug**: Because properties like `hunger` and `fuel` decay in real-time in Godot, the snapshots taken at the start of planning would have tiny floating-point differences. This made the Visited Set ineffective, as it treated nearly identical states as unique.
- **Attempted Fix**: Limited hashing to "critical" properties like `held_item` and normalized positions.
- **Learning**: Planning state should likely be decoupled from "noisy" simulation state. We should only hash properties that actions *explicitly* modify or require.

### 3. Heuristic Scale Mismatch (The "Distance Trap")
A* depends on an "admissible" heuristic ($h \leq h^*$).
- **Bug**: Interaction actions had costs like `0.2` or `1.0`. `GoToAction` had raw Euclidean distance costs (e.g., `500.0`). The heuristic estimated navigation at `10.0`.
- **Result**: A* would explore every possible deep combination of interactions (total cost ~15.0) before ever trying a single `GoTo` action (cost 500.0). This made A* perform worse than BFS/DFS.
- **Attempted Fix**: Normalized navigation costs (e.g., `dist / 100.0`).
- **Learning**: Heuristics and action costs must operate on the same magnitude.

### 4. Validation De-sync
The `forward_validate` pass was manually re-implementing simulation logic that differed from the `simulate_chain` and `simulate_effect` logic used during expansion.
- **Bug**: Bindings (target objects) were injected into properties but weren't being correctly registered as symbolic provisions for later steps in the forward pass.
- **Learning**: The forward validation pass should be a strict application of the *exact same* simulation code used during search expansion.

## Recommendations for Phase 2 Retry
1. **Unify Simulation**: Use a single path for state mutation during both expansion and validation.
2. **Simplified Symbolic State**: Define a "Planning State" that is a subset of the "Blackboard State," excluding noisy/continuous values unless they are the subject of a goal.
3. **Implicit Requirements**: Improve the bridge so that interaction actions automatically inherit requirements from their target's group/type.
4. **Diagnostic Tooling**: Keep the "Search Tree Dump" as it was vital for seeing *why* plans were being rejected at depth.
