# Planner Strategy Modularization Plan

**Date:** 2026-05-16
**Status:** Planning phase

## Motivation

The current backward-chaining planner in `planner.rs` uses a single hardcoded recursive DFS with a hardcoded termination heuristic (exhaustive search within `max_depth`). We have observed two contradictory failure modes:

1. **Early-exit** (`return Some(result)` on first valid plan): terminates fast but may return a suboptimal plan.
2. **Exhaustive search** (`best_result` tracking + `<=` comparison): finds the cheapest plan among explored branches but can feel "endless" in scenes with many action candidates because it explores the full combinatorial space with no pruning until a valid plan is found.

We need to be able to swap search strategies and termination conditions independently, both for interactive debugging and for production deployment. This will also enable fair benchmark comparisons (DFS vs A* vs Dijkstra vs beam search) and per-agent configurability (e.g., a background worker agent can afford exhaustive search, a real-time NPC needs a 20ms time cap).

---

## Design Overview

### Guiding Principles

1. **Trait-based separation**: The core branch expansion logic (what constitutes a successor branch) stays shared. The *order* in which branches are explored and the *condition* under which exploration stops become pluggable.
2. **No premature abstraction**: Start by extracting the search loop into an iterative structure, then introduce traits.
3. **Benchmarkable by default**: Every strategy implementation must expose `SearchStats` so we can compare branches explored, time elapsed, and plan cost.
4. **GDScript-configurable**: The agent config resource gets new fields for strategy selection.

### Architecture

All planner logic moves from `planner.rs` into a `planner/` directory:

```
src/planner/
├── mod.rs                  (public re-exports, legacy backward_search wrapper during migration)
├── engine.rs               (PlannerEngine — orchestrates expander + controller + policy)
├── expander.rs             (BranchExpander — shared successor generation)
├── controller.rs           (SearchController trait + DfsController + AStarController + DijkstraController)
├── policy.rs               (TerminationPolicy trait + FirstValidPolicy + ExhaustivePolicy + BudgetPolicy)
├── stats.rs                (SearchStats + benchmark helpers)
└── heuristic.rs            (admissible heuristics for A*)
```

`lib.rs` keeps `pub mod planner;` and the public API surface does not change.

---

## 1. Core Data Structures

### `SearchNode`

Replace the implicit recursion stack with an explicit node:

```rust
struct SearchNode {
    branch: PlanBranch,
    depth: usize,
    accumulated_cost: f64,      // g(n): cost from goal to this node
    estimated_remaining: f64,   // h(n): heuristic estimate to a complete plan
}
```

`accumulated_cost` is the sum of estimated costs of actions added so far (the suffix chain). `estimated_remaining` is a heuristic on the open needs (e.g., count of unsatisfied preconditions × min action cost).

### `SearchStats`

```rust
#[derive(Debug, Clone)]
struct SearchStats {
    branches_expanded: usize,
    branches_pruned: usize,
    max_depth_reached: usize,
    valid_plans_found: usize,
    best_cost: f64,
    start_time: Instant,
    elapsed_ms: u64,
}
```

---

## 2. BranchExpander (Shared Logic)

Extract everything from `backward_search` that *generates* successors into a standalone `BranchExpander`:

```rust
struct BranchExpander<'ctx> {
    ctx: &'ctx SearchContext,
}

impl<'ctx> BranchExpander<'ctx> {
    /// Returns all successor branches for a given node.
    /// Each successor = this branch + one predecessor action inserted.
    fn expand(&self, node: &SearchNode) -> Vec<SearchNode> {
        // 1. Check if node.branch.is_complete() -> return empty (leaf)
        // 2. find_candidate_actions()
        // 3. For each candidate:
        //    a. Clone branch
        //    b. insertion_index_for_candidate()
        //    c. shift_branch_positions_for_insert()
        //    d. insert action into chain
        //    e. call_apply_effect (accumulated_agent/world)
        //    f. update_open_needs()
        //    g. Compute new accumulated_cost
        //    h. Return SearchNode
    }
}
```

Key benefit: `expand` is pure-ish (no mutable global state) and unit-testable in isolation.

---

## 3. SearchController Trait

```rust
trait SearchController {
    /// Push an initial node (the goal branch) into the frontier.
    fn push_initial(&mut self, node: SearchNode);

    /// Pop the next node to explore. Returns None when frontier is empty.
    fn pop_next(&mut self) -> Option<SearchNode>;

    /// Push a set of successors back into the frontier.
    fn push_successors(&mut self, successors: Vec<SearchNode>);

    /// Current frontier size (for stats).
    fn frontier_size(&self) -> usize;
}
```

### 3.1 `DfsController`

- Frontier: `Vec<SearchNode>` (stack).
- `pop_next` returns `pop()` (LIFO).
- `push_successors` pushes all successors; they will be explored depth-first.
- Optional: `depth_limited` flag to stop pushing successors when `depth >= max_depth`.

### 3.2 `AStarController`

- Frontier: `BinaryHeap<SearchNode>` ordered by `f = accumulated_cost + estimated_remaining` (min-heap).
- Requires `SearchNode: Ord` where ordering is by `f`.
- Also tracks a `visited: HashSet<BranchFingerprint>` to avoid re-expanding equivalent branches.
- **Heuristic**: `estimated_remaining` can be a simple admissible heuristic like `open_preconditions.len() * min_action_cost` or `open_requirements.len() * min_provision_cost`.

### 3.3 `DijkstraController`

- Same as A* but `estimated_remaining = 0.0` (no heuristic).
- Guarantees the first valid plan found is the cheapest (if `accumulated_cost` is monotonic and the heuristic is admissible).
- In our domain, `accumulated_cost` comes from `estimate_action_cost`, which is a lower bound. If we use Dijkstra with a lower-bound cost, the first complete branch popped from the priority queue is the cheapest plan.

### 3.4 `BeamController` (future)

- Same priority queue as A* but `push_successors` only keeps the top K nodes.
- Sacrifices completeness for speed.

---

## 4. TerminationPolicy Trait

```rust
trait TerminationPolicy {
    /// Called once at search start.
    fn on_search_start(&mut self, max_depth: usize);

    /// Called after each pop from the frontier. Return true to stop searching.
    fn should_terminate(&self, stats: &SearchStats, controller: &dyn SearchController) -> bool;

    /// Called when a complete (valid) plan is found.
    fn on_valid_plan_found(&mut self, cost: f64, stats: &SearchStats);
}
```

### 4.1 `FirstValidPolicy`

- `should_terminate`: returns `true` as soon as `valid_plans_found > 0`.
- Effect: stop on the first valid plan encountered by the search controller's ordering.
  - With DFS: first valid plan found in DFS order.
  - With A*: first valid plan found in f-order (if heuristic is admissible, this is optimal).

### 4.2 `ExhaustivePolicy`

- `should_terminate`: returns `true` only when frontier is empty or `max_depth` is exhausted for all frontier nodes.
- Effect: explores the entire reachable space.

### 4.3 `BudgetPolicy`

- Configurable with:
  - `max_time_ms: u64`
  - `max_branches: usize`
- `should_terminate`: returns `true` when `elapsed_ms >= max_time_ms` OR `branches_expanded >= max_branches`.
- Effect: bounded effort. Returns the best plan found so far (if any).

### 4.4 `BestWithinBudgetPolicy` (recommended default for production)

- Same as `BudgetPolicy` but **also** terminates early if `valid_plans_found > 0` AND `elapsed_ms >= max_time_ms`.
- Effect: keep searching until time runs out, continuously improving the best plan.

---

## 5. PlannerEngine (Orchestrator)

```rust
struct PlannerEngine {
    expander: BranchExpander,
    controller: Box<dyn SearchController>,
    policy: Box<dyn TerminationPolicy>,
    stats: SearchStats,
}

impl PlannerEngine {
    fn run(&mut self, goal_branch: PlanBranch, max_depth: usize) -> Option<PlanResult> {
        self.policy.on_search_start(max_depth);
        self.controller.push_initial(SearchNode {
            branch: goal_branch,
            depth: 0,
            accumulated_cost: 0.0,
            estimated_remaining: 0.0,
        });

        let mut best_result: Option<PlanResult> = None;
        let mut best_cost = f64::INFINITY;

        while let Some(node) = self.controller.pop_next() {
            self.stats.branches_expanded += 1;
            self.stats.max_depth_reached = self.stats.max_depth_reached.max(node.depth);

            if self.policy.should_terminate(&self.stats, self.controller.as_ref()) {
                break;
            }

            // Check completeness
            if is_complete(&node.branch, self.expander.ctx) {
                if let Some(result) = forward_validate(&node.branch, self.expander.ctx) {
                    if result.1 < best_cost {
                        best_cost = result.1;
                        best_result = Some(result);
                        self.policy.on_valid_plan_found(best_cost, &self.stats);
                        self.stats.best_cost = best_cost;
                        self.stats.valid_plans_found += 1;
                    }
                }
                continue;
            }

            // Prune by depth and cost
            if node.depth >= max_depth {
                self.stats.branches_pruned += 1;
                continue;
            }
            if node.accumulated_cost + node.estimated_remaining >= best_cost {
                self.stats.branches_pruned += 1;
                continue;
            }

            let successors = self.expander.expand(&node);
            self.controller.push_successors(successors);
        }

        best_result
    }
}
```

Key improvements over current code:
- **Iterative**, not recursive — no stack overflow risk, easy to add breakpoints.
- **Controller handles ordering** — DFS, A*, Dijkstra all use the same loop body.
- **Policy handles stopping** — time budgets, branch budgets, first-valid, exhaustive all use the same loop body.
- **Cost-based pruning** happens before expansion: if `g + h >= best_cost`, skip.

---

## 6. Per-Agent Configuration

### AgentConfig additions

```gdscript
# gdpai_agent_config.gd

enum SearchStrategy {
    DFS = 0,
    A_STAR = 1,
    DIJKSTRA = 2,
}

enum TerminationStrategy {
    FIRST_VALID = 0,
    EXHAUSTIVE = 1,
    TIME_BUDGET = 2,
    BEST_WITHIN_BUDGET = 3,
}

@export var search_strategy: SearchStrategy = SearchStrategy.DFS
@export var termination_strategy: TerminationStrategy = TerminationStrategy.FIRST_VALID
@export var max_search_time_ms: int = 50          # For TIME_BUDGET / BEST_WITHIN_BUDGET
@export var max_search_branches: int = 10000       # Hard cap on branches explored
@export var beam_width: int = 0                     # 0 = unlimited (for future Beam search)
```

### Bridge/Scheduler

The `SearchContext` gets a new `strategy_config` field. The `run_plan` function constructs the appropriate `Box<dyn SearchController>` and `Box<dyn TerminationPolicy>` based on the config.

```rust
// In scheduler.rs or planner/mod.rs
fn build_controller(config: &StrategyConfig) -> Box<dyn SearchController> {
    match config.search_strategy {
        SearchStrategy::Dfs => Box::new(DfsController::new()),
        SearchStrategy::AStar => Box::new(AStarController::new(config.beam_width)),
        SearchStrategy::Dijkstra => Box::new(DijkstraController::new()),
    }
}

fn build_policy(config: &StrategyConfig) -> Box<dyn TerminationPolicy> {
    match config.termination_strategy {
        TerminationStrategy::FirstValid => Box::new(FirstValidPolicy::new()),
        TerminationStrategy::Exhaustive => Box::new(ExhaustivePolicy::new()),
        TerminationStrategy::TimeBudget => Box::new(BudgetPolicy::new(config.max_search_time_ms, config.max_search_branches)),
        TerminationStrategy::BestWithinBudget => Box::new(BestWithinBudgetPolicy::new(config.max_search_time_ms, config.max_search_branches)),
    }
}
```

---

## 7. Benchmarking

Create `rust/benches/planner_benches.rs` (or `test/benchmarks/`) that runs the same scenario against every strategy combination:

```rust
fn benchmark_campfire_scene() {
    let scenarios = vec![
        ("campfire_depth4", campfire_actions(), campfire_goal(), 4),
        ("campfire_depth6", campfire_actions(), campfire_goal(), 6),
        ("campfire_depth8", campfire_actions(), campfire_goal(), 8),
    ];

    let strategies = vec![
        ("dfs_first", SearchStrategy::Dfs, TerminationStrategy::FirstValid),
        ("dfs_exhaustive", SearchStrategy::Dfs, TerminationStrategy::Exhaustive),
        ("astar_first", SearchStrategy::AStar, TerminationStrategy::FirstValid),
        ("astar_budget50ms", SearchStrategy::AStar, TerminationStrategy::BestWithinBudget),
        ("dijkstra_exhaustive", SearchStrategy::Dijkstra, TerminationStrategy::Exhaustive),
    ];

    for (scenario_name, actions, goal, depth) in &scenarios {
        for (strategy_name, search, termination) in &strategies {
            let stats = run_planner(actions, goal, depth, search, termination);
            println!("{}/{}: cost={}, branches={}, time={}ms, valid={}",
                scenario_name, strategy_name,
                stats.best_cost, stats.branches_expanded,
                stats.elapsed_ms, stats.valid_plans_found);
        }
    }
}
```

Metrics to collect:
- `plan_cost` (lower is better)
- `branches_expanded` (lower is better for efficiency)
- `elapsed_ms` (lower is better for latency)
- `valid_plans_found` (higher means more exploration)
- `success_rate` (% of scenarios that found a plan at all)

---

## 8. Implementation Phases

### Phase 1: Create `planner/` directory + Iterative DFS
**Goal:** Replace recursive `backward_search` with iterative loop, keep DFS behavior.
- Create `src/planner/` directory.
- Move candidate-finding, branch-cloning, and open-need-update logic into `planner/expander.rs` as `BranchExpander`.
- Implement `DfsController` in `planner/controller.rs` (Vec stack).
- Implement `FirstValidPolicy` and `ExhaustivePolicy` in `planner/policy.rs`.
- Implement `PlannerEngine` in `planner/engine.rs` with the iterative loop.
- Convert `planner.rs` to a shim that re-exports `PlannerEngine::run_plan` so existing callers in `scheduler.rs` compile unchanged.
- **Validation:** `make test-rust`, `make test-godot`. No behavior change expected.

### Phase 2: Add Cost-Based Pruning
**Goal:** Make exhaustive search practical.
- In the main loop, before expanding a node, check `node.accumulated_cost + estimated_remaining >= best_cost`.
- This alone should make the current campfire scene with `ExhaustivePolicy` terminate in < 1 second instead of "endless".
- **Validation:** Interactive campfire test with `TerminationStrategy::Exhaustive`.

### Phase 3: Implement A* and Dijkstra Controllers
**Goal:** Add informed search strategies.
- Implement `AStarController` in `planner/controller.rs` with `BinaryHeap`.
- Design admissible heuristic in `planner/heuristic.rs` for `estimated_remaining`.
- Implement `DijkstraController` in `planner/controller.rs` (A* with h=0).
- **Validation:** Benchmark Phase 1 scenarios against new strategies. A* should find cheaper plans than DFS-first-valid with fewer branches.

### Phase 4: Implement Budget Policies
**Goal:** Make search configurable for real-time use.
- Implement `BudgetPolicy` and `BestWithinBudgetPolicy` in `planner/policy.rs`.
- Add `max_search_time_ms` and `max_search_branches` to agent config.
- Serialize these through the bridge to the Rust side.
- **Validation:** Run campfire scene with `max_search_time_ms = 20` and verify agent still acts intelligently.

### Phase 5: Benchmark Suite + GDScript Exposé
**Goal:** Surface stats to Godot for debugging.
- Add `SearchStats` to the plan result dictionary.
- Expose a GDScript API to query last search stats (branches, time, cost).
- Create benchmark integration test that runs a matrix of strategies.
- **Validation:** `make test-godot` includes a benchmark scenario.

---

## 9. Resolved Questions

1. **Heuristic for A*:** **Decision:** Use `count(open_preconditions) * min_action_cost_in_catalog` as the admissible heuristic, following standard GOAP practice. This is cheap to compute and guaranteed not to overestimate. We can improve it later if benchmarks show A* is exploring too many nodes.

2. **Visited-state deduplication:** **Decision:** Likely not possible in this domain because different branch chains (different action sequences) can produce the same open needs but with different accumulated costs and action bindings. However, we will investigate cost-based pruning first; if that is insufficient, we can experiment with a `HashSet<BranchFingerprint>` in `AStarController` as an optional optimization.

3. **Beam search worth it?** **Decision:** Defer. The modular architecture makes it trivial to add a `BeamController` later. Cost-based pruning (Phase 2) may already provide enough of the benefit.

4. **Probability-based pruning:** **Decision:** Do not implement at start. `forward_validate` remains the ground truth. We will rely on cost-based pruning and admissible heuristics rather than probabilistic estimates.

---

## 10. Files to Create / Modify

### New files
- `addons/GdPlanningAI/rust/src/planner/mod.rs` — module root, re-exports public API
- `addons/GdPlanningAI/rust/src/planner/engine.rs` — `PlannerEngine` + orchestration loop
- `addons/GdPlanningAI/rust/src/planner/expander.rs` — `BranchExpander`
- `addons/GdPlanningAI/rust/src/planner/controller.rs` — `SearchController` trait + `DfsController`, `AStarController`, `DijkstraController`
- `addons/GdPlanningAI/rust/src/planner/policy.rs` — `TerminationPolicy` trait + `FirstValidPolicy`, `ExhaustivePolicy`, `BudgetPolicy`, `BestWithinBudgetPolicy`
- `addons/GdPlanningAI/rust/src/planner/stats.rs` — `SearchStats`
- `addons/GdPlanningAI/rust/src/planner/heuristic.rs` — admissible heuristics for A*
- `addons/GdPlanningAI/rust/benches/planner_benches.rs` — Criterion benchmarks

### Modified files
- `addons/GdPlanningAI/rust/src/planner.rs` → becomes a thin re-export shim during migration, then deleted once all callers import from `planner::`
- `addons/GdPlanningAI/rust/src/lib.rs` — update module declarations if needed (likely just keep `pub mod planner;`)
- `addons/GdPlanningAI/rust/src/scheduler.rs` — construct controller/policy from config, forward stats
- `addons/GdPlanningAI/scripts/resources/gdpai_agent_config.gd` — add strategy enum fields
- `addons/GdPlanningAI/scripts/gdpai_rust_bridge.gd` — serialize new config fields

### Deleted files
- `addons/GdPlanningAI/rust/src/planner.rs` — remove in Phase 2 after all imports migrate to `planner::`.

---

## Acceptance Criteria

- [ ] All existing Rust tests pass after Phase 1.
- [ ] All existing Godot tests pass after Phase 1.
- [ ] A benchmark can run the campfire scene with `DFS + FirstValid`, `DFS + Exhaustive`, `A* + FirstValid`, and `A* + BestWithinBudget(50ms)` and print comparable stats.
- [ ] An agent config can select a strategy, and the planner respects it.
- [ ] The planner does not feel "endless" in the campfire scene with `ExhaustivePolicy` (target: < 2 seconds for depth 6).

---

## Next Step

Review this plan. If approved, we begin Phase 1: extracting `BranchExpander` and converting the recursive DFS to an iterative loop while preserving existing behavior.
