# Phase 2 Retry: Heuristic-Augmented Pruning & Simulation Unity

## Core Objectives
1. **Simulation Unity**: Ensure that the forward validation pass uses the same logic as the search expansion's state accumulation.
2. **Robust Heuristics**: Implement admissible heuristics that are scaled correctly with action costs.
3. **Symbolic Pruning**: Use requirements more effectively to prune invalid branches early (the "Eat Wood" problem).
4. **Iterative A***: Safely introduce A* search once the underlying simulation and cost model are stable.

## Granular Tasks

### Task 1: Simulation Unity & Foundation
- [x] Refactor `forward_validate` to leverage `BranchExpander` logic or shared utilities for state mutation.
- [x] Ensure `action_bindings` are applied consistently in both expansion and validation.
- [x] Add a "Planning State" projection to `BlackboardSnapshot` (or a helper) that ignores noisy/float properties for hashing/comparison purposes.

### Task 2: Heuristic & Cost Calibration
- [x] Calculate `min_action_cost` and `min_provision_cost` dynamically from the action catalog at search start instead of hardcoding to 1.0.
- [x] Update `heuristic::estimate_remaining` to use these dynamic minimums.
- [ ] Address the "Distance Trap": Ensure `GoToAction` costs and interaction costs are on similar scales (e.g., normalize distance-based costs).

### Task 3: Symbolic Pruning (Fixing "Eat Wood")
- [ ] Strengthen `RequirementSpec::BindingInSet` check during backward expansion.
- [ ] Ensure that if an action has a requirement that cannot be satisfied by any provider or initial state, it is pruned immediately.
- [ ] Implement early rejection for actions whose symbolic provisions contradict the goal or intermediate needs.

### Task 4: A* Search Implementation
- [x] Implement `AStarController` using `BinaryHeap`.
- [x] Verify `SearchNode` implements `Ord` and `PartialOrd` correctly (min-heap by `f = g + h`).
- [x] Implement state deduplication (visited set) using `StableSnapshot`.
- [ ] Test A* against DFS on the campfire scene to verify it finds the same (or better) plans with fewer branches.

### Task 5: Budget & Stats
- [ ] Implement `BudgetPolicy` (time/branch limits).
- [ ] Expose search stats (branches expanded, time taken, pruning count) back to Godot.
- [ ] Add interactive debugging support for viewing the search tree with pruning reasons.
