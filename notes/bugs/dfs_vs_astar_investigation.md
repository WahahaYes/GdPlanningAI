# DFS vs A* Investigation

**Date:** 2026-05-19

**Context:**
Planner is timing out on complex scenarios (campfire, hunger examples). User mentioned DFS used to work, but current A* implementation is getting stuck.

**Git History:**
- Commit `02a0a04` "breakout planner into modular components" is where the planner was refactored
- Old: Single large `planner.rs` (1555 lines)
- New: Modular components (controller.rs, engine.rs, expander.rs, heuristic.rs, etc.)

## Old DFS Implementation (pre-02a0a04)

**Algorithm:** Depth-First Search with cost-based pruning

**Key Characteristics:**
```rust
fn backward_search(
    branch: PlanBranch,
    ctx: &SearchContext,
    depth: usize,
    best_cost: &mut f64,
) -> Option<(Vec<i64>, f64, Vec<(i64, String, Vec<i64>)>)>
```

1. **Depth-based recursion**: Uses `depth` parameter, recursive calls with `depth + 1`
2. **Cost-based pruning**: `if branch.estimated_cost >= *best_cost` - prunes branches that exceed best known cost
3. **Early termination**: Returns first complete plan found (not necessarily optimal)
4. **Candidate sorting**: Candidates sorted by estimated cost before trying
5. **No visited set**: Allows revisiting states (may explore same state multiple times)

**Search Strategy:**
- Depth-first: Go deep into one branch before backtracking
- Pruning: Cut off branches that are already more expensive than best solution found
- Returns first valid plan: Not guaranteed to be optimal, but fast

## New A* Implementation (post-02a0a04)

**Algorithm:** A* Search with heuristic

**Key Characteristics:**
```rust
fn search_goal(&mut self, goal: &GoalSpec) -> Option<PlanResult> {
    let mut controller = AStarController::new();
    // ...
    while let Some(node) = controller.pop() {
        // ...
        let successors = expander.expand(&node.branch);
        for succ in successors {
            let h = heuristic::estimate_remaining(&succ, self.ctx.actions);
            controller.push(SearchNode { ... });
        }
    }
}
```

1. **Best-first with priority queue**: Uses `BinaryHeap` ordered by `cost + estimated_remaining`
2. **Heuristic guidance**: Uses heuristic function to estimate remaining cost
3. **Visited set**: Uses `HashSet<NodeFingerprint>` to avoid revisiting states
4. **Exhaustive search**: Explores all branches within max depth to find optimal plan
5. **Order-dependent fingerprint**: Visited set uses vector ordering for `open_preconditions` and `open_requirements`

**Search Strategy:**
- Best-first: Always explore most promising node first (lowest cost + heuristic)
- Optimality: Explores entire search space to guarantee optimal solution
- Visited set: Prevents revisiting equivalent states

## Key Differences

| Aspect | Old DFS | New A* |
|--------|---------|--------|
| Search order | Depth-first | Best-first (priority queue) |
| Plan quality | First valid plan (suboptimal) | Optimal plan |
| Visited set | None (allows revisits) | HashSet (prevents revisits) |
| Cost pruning | Yes (branch cost >= best) | No (heuristic guidance) |
| Termination | First complete plan | Exhaustive within max depth |
| Heuristic | None | Yes (estimate_remaining) |

## Root Cause of Timeouts

The new A* implementation is timing out because:

1. **Exhaustive search**: Explores entire search space to find optimal plan, not just first valid plan
2. **Weak heuristic**: Current heuristic is too weak, causing poor search guidance
3. **Visited set strictness**: Order-dependent vector comparison may cause state revisits or prevent finding valid plans
4. **Search space explosion**: With many object-provided actions and requirements/provisions, the search space is too large

## Why DFS Worked

DFS worked because:
1. **Early termination**: Returns first valid plan, doesn't need to explore entire space
2. **Cost-based pruning**: Cuts off branches that are already too expensive
3. **No visited set**: Allows exploring multiple paths to same state (may find solution faster)
4. **Depth-first**: Goes deep quickly, can find solutions in narrow search spaces

## Potential Solutions

1. **Hybrid approach**: Use A* with depth limiting or beam search
2. **Improve heuristic**: Better heuristic for search guidance
3. **Relax visited set**: Use order-independent comparison for fingerprints
4. **Revert to DFS**: With better pruning strategies
5. **Iterative deepening**: Combine DFS with iterative deepening A* (IDA*)
