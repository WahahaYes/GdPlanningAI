# Planner Strategy Modularization Plan

**Date:** 2026-05-16
**Status:** Planning phase

## Motivation

The current backward-chaining planner in `planner.rs` uses a single hardcoded recursive DFS with a hardcoded termination heuristic (exhaustive search within `max_depth`). We have observed two contradictory failure modes:

1. **Early-exit** (`return Some(result)` on first valid plan): terminates fast but may return a suboptimal plan.
2. **Exhaustive search** (`best_result` tracking + `<=` comparison): finds the cheapest plan among explored branches but can feel "endless" in scenes with many action candidates because it explores the full combinatorial space with no pruning until a valid plan is found.

We need to be able to swap search strategies, termination conditions, and goal selection policies independently, both for interactive debugging and for production deployment. This will also enable fair benchmark comparisons (DFS vs A* vs Dijkstra vs beam search) and per-agent configurability (e.g., a background worker agent can afford exhaustive search, a real-time NPC needs a 20ms time cap).

---

## Design Overview

### Guiding Principles

1. **Trait-based separation**: The core branch expansion logic (what constitutes a successor branch) stays shared. The *order* in which branches are explored, the *condition* under which exploration stops, and the *policy* for selecting and iterating goals all become pluggable.
2. **No premature abstraction**: Start by extracting the search loop into an iterative structure, then introduce traits.
3. **Benchmarkable by default**: Every strategy implementation must expose `SearchStats` so we can compare branches explored, time elapsed, and plan cost.
4. **GDScript-configurable**: The agent config resource gets new fields for strategy selection.

### Architecture

All planner logic moves from `planner.rs` into a `planner/` directory:

```
src/planner/
├── mod.rs                  (public re-exports, legacy backward_search wrapper during migration)
├── engine.rs               (PlannerEngine — orchestrates expander + controller + policy + goal_selection)
├── expander.rs             (BranchExpander — shared successor generation)
├── controller.rs           (SearchController trait + DfsController + AStarController + DijkstraController)
├── policy.rs               (TerminationPolicy trait + FirstValidPolicy + ExhaustivePolicy + BudgetPolicy)
├── goal_selection.rs       (GoalSelection trait + HighestRewardFirst + AllGoalsBestPlan)
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
    estimated_remaining: f64,   // h(n): heuristic estimate to a complete plan
}
```

- **`branch.estimated_cost`** is g(n): the sum of lower-bound estimated costs of actions added so far (the suffix chain). This field already exists on `PlanBranch` and is maintained by the expander.
- **`estimated_remaining`** is h(n): an admissible heuristic estimate of the cost to satisfy all remaining open needs (both preconditions and requirements). See [§ heuristic.rs](#-heuristicrs-design-) for design.
- **`depth`** tracks recursion depth for depth-limited pruning (equivalent to the current `depth` parameter in `backward_search`).

### `SearchStats`

```rust
#[derive(Debug)]
struct SearchStats {
    branches_expanded: usize,
    branches_pruned: usize,
    max_depth_reached: usize,
    valid_plans_found: usize,
    best_cost: f64,
    start_time: Instant,
}

impl SearchStats {
    fn elapsed_ms(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }
}
```

`elapsed_ms` is a computed method, not a stored field — it derives from `start_time.elapsed()` at query time.

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
    /// Returns empty vec for complete branches (leaf nodes).
    fn expand(&self, node: &SearchNode) -> Vec<SearchNode> {
        // 1. If node.branch.is_complete(ctx.request_tx) -> return empty (leaf)
        // 2. candidates = find_candidate_actions(&node.branch, ctx)
        // 3. For each candidate:
        //    a. new_branch = node.branch.clone()
        //    b. insert_pos = insertion_index_for_candidate(&new_branch, &candidate)
        //    c. shift_branch_positions_for_insert(&mut new_branch, insert_pos)
        //    d. new_branch.action_chain.insert(insert_pos, candidate.action_idx)
        //    e. new_branch.estimated_cost += candidate.estimated_cost
        //    f. update_open_needs(&mut new_branch, &candidate, action, insert_pos, ctx)
        //       NOTE: This is ~140 lines of complex logic handling wildcard bindings,
        //       pending effects, bound provisions, and requirement consumer tracking.
        //       It lives in expander.rs but is the most intricate part of the extraction.
        //    g. If requirements met: call_apply_effect → update accumulated_agent/world
        //    h. Compute estimated_remaining via heuristic
        //    i. Push SearchNode { branch: new_branch, depth: node.depth + 1, estimated_remaining }
    }
}
```

Key benefit: `expand` is pure-ish (no mutable global state) and unit-testable in isolation.

**Extraction note:** `update_open_needs` (currently ~140 lines in `planner.rs`) is the most complex function being extracted. It handles:
- Wildcard `FactWildcard` provision → concrete `Fact` provision binding with chain-position tracking
- `action_bindings` accumulation for later injection into `forward_validate`
- `pending_effects` for requirement-dependent state effects that need re-simulation after providers are bound
- `resolve_pending_effects` re-simulation loop
- Duplicate precondition detection via `preconditions_equal`
- Adding the action's own requirements and preconditions as new open needs

This function and its callees (`resolve_pending_effects`, `remove_preconditions_by_index`, `preconditions_equal`) should stay together in `expander.rs`.

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
- **Heuristic**: `estimated_remaining` can be a simple admissible heuristic like `open_preconditions.len() * min_action_cost` or `open_requirements.len() * min_provision_cost`.
- Note: visited-state deduplication is deferred (see [§ Resolved Questions](#-resolved-questions)).

### 3.3 `DijkstraController`

- Same as A* but `estimated_remaining = 0.0` (no heuristic).
- Explores nodes in order of `g(n)` (accumulated lower-bound cost). The first complete plan popped is cheapest *by lower-bound estimate*, but the true forward-validated cost may differ because action costs depend on concrete bindings resolved during `forward_validate`. Therefore Dijkstra does **not** guarantee the first valid plan is truly optimal — it only guarantees optimality with respect to the lower-bound estimates used during search.
- Still useful as a baseline: it explores cheap-looking branches first, which empirically finds good plans early.

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

## 5. GoalSelection Trait

Goal iteration is currently hardcoded in `run_plan`: sort goals by reward descending, return the first goal that yields a valid plan, and short-circuit with an empty plan if a goal is already satisfied. This should be pluggable.

All strategies share a common threshold: **`max_goals_to_consider: usize`** (0 = unlimited). Before any strategy-specific logic runs, the goal list is truncated to the top N goals by reward. This bounds planning effort regardless of strategy choice.

```rust
trait GoalSelection {
    /// Called once at the start of planning with the full goal list.
    /// Returns the ordered sequence of candidates to try,
    /// and for each whether to skip the goal if it is already satisfied.
    fn select_goals(&self, goals: &[GoalSpec]) -> Vec<GoalCandidate>;

    /// If true, the engine stops after the first goal that yields any valid plan
    /// (including an already-satisfied goal with an empty plan).
    fn short_circuit_on_first_valid(&self) -> bool;

    /// Compare two (reward, cost) pairs. Return true if candidate `a` is strictly
    /// better than candidate `b` according to this strategy's metric.
    /// Used by the engine to track the best result across multiple goals.
    fn is_better_than(&self, reward_a: f64, cost_a: f64, reward_b: f64, cost_b: f64) -> bool;
}

struct GoalCandidate {
    /// Index into the original goals slice.
    goal_index: usize,
    /// If true and the goal's desired_state is already satisfied by the
    /// initial agent/world state, skip this goal entirely (no planning).
    /// If false, always attempt to plan for this goal even if satisfied.
    skip_if_satisfied: bool,
}
```

### 5.1 `HighestRewardFirst`

- `short_circuit_on_first_valid() = true`
- `is_better_than` compares by reward (higher is better), tie-breaking on lower cost.
- Sorts goals by reward descending, truncates to `max_goals_to_consider`.
- `skip_if_satisfied = true` for all goals.
- This is the default — matches existing semantics exactly when `max_goals_to_consider = 0`.

### 5.2 `HighestRewardFirstNoSkip`

- `short_circuit_on_first_valid() = true`
- `is_better_than` compares by reward (higher is better), tie-breaking on lower cost.
- Same ordering and truncation as `HighestRewardFirst` but `skip_if_satisfied = false`.
- Always attempts to plan, even for goals whose preconditions are already met.

### 5.3 `AllGoalsBestPlan`

- `short_circuit_on_first_valid() = false`
- `is_better_than` compares by cost (lower is better), tie-breaking on higher reward.
- Considers up to `max_goals_to_consider` goals (in reward order).
- `skip_if_satisfied = true`.

### 5.4 `AllGoalsBestPlanNoSkip`

- `short_circuit_on_first_valid() = false`
- `is_better_than` compares by cost (lower is better), tie-breaking on higher reward.
- Like `AllGoalsBestPlan` but `skip_if_satisfied = false`.

### 5.5 `BestRewardCostRatio`

- `short_circuit_on_first_valid() = false`
- `is_better_than` compares by `reward / cost` ratio (higher is better), tie-breaking on higher reward.
- Considers up to `max_goals_to_consider` goals (in reward order).
- `skip_if_satisfied = true`.
- Balances goal importance against plan effort — a high-reward goal with an expensive plan may lose to a moderate-reward goal with a cheap plan.

---

## 6. PlannerEngine (Orchestrator)

```rust
struct PlannerEngine<'ctx> {
    ctx: &'ctx SearchContext,
    controller: Box<dyn SearchController>,
    policy: Box<dyn TerminationPolicy>,
    goal_selection: Box<dyn GoalSelection>,
    stats: SearchStats,
}

impl<'ctx> PlannerEngine<'ctx> {
    /// Run planning for all goals according to the goal selection strategy.
    /// Returns the best PlanResult across selected goals, or None.
    fn run(&mut self) -> Option<PlanResult> {
        let mut best_result: Option<PlanResult> = None;
        let mut best_reward: f64 = 0.0;
        let mut best_cost: f64 = f64::INFINITY;

        let candidates = self.goal_selection.select_goals(self.ctx.goals);

        for candidate in &candidates {
            if self.ctx.cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                return best_result;
            }

            let goal = &self.ctx.goals[candidate.goal_index];

            // Check if goal is already satisfied
            let goal_satisfied = goal.desired_state.iter().all(|p| {
                p.evaluate_builtin(self.ctx.initial_agent, self.ctx.initial_world)
                    .unwrap_or_else(|| eval_precondition(p, self.ctx.initial_agent, self.ctx.initial_world, self.ctx.request_tx))
            });

            if goal_satisfied && candidate.skip_if_satisfied {
                let result = PlanResult {
                    success: true,
                    action_chain: vec![],
                    total_cost: 0.0,
                    goal_index: goal.original_index as i64,
                    deferred_action_indices: vec![],
                    action_bindings: vec![],
                };
                if self.goal_selection.short_circuit_on_first_valid() {
                    return Some(result);
                }
                if best_result.is_none() || self.goal_selection.is_better_than(goal.reward, 0.0, best_reward, best_cost) {
                    best_reward = goal.reward;
                    best_cost = 0.0;
                    best_result = Some(result);
                    self.stats.best_cost = 0.0;
                }
                continue;
            }

            // Build root branch for this goal
            let root_branch = PlanBranch::new(
                &goal.desired_state,
                self.ctx.initial_provisions,
                self.ctx.initial_agent,
                self.ctx.initial_world,
            );

            // Run search for this goal
            if let Some(result) = self.search_goal(root_branch, goal.original_index) {
                if self.goal_selection.short_circuit_on_first_valid() {
                    return Some(result);
                }
                if best_result.is_none() || self.goal_selection.is_better_than(goal.reward, result.total_cost, best_reward, best_cost) {
                    best_reward = goal.reward;
                    best_cost = result.total_cost;
                    best_result = Some(result);
                    self.stats.best_cost = best_cost;
                }
            }
        }

        best_result
    }

    /// Search backward from a single goal branch.
    fn search_goal(&mut self, goal_branch: PlanBranch, goal_index: usize) -> Option<PlanResult> {
        self.policy.on_search_start(self.ctx.max_depth);
        self.controller.push_initial(SearchNode {
            branch: goal_branch,
            depth: 0,
            estimated_remaining: 0.0,
        });

        let mut best_result: Option<PlanResult> = None;
        let mut best_cost = f64::INFINITY;

        while let Some(node) = self.controller.pop_next() {
            // Cancel check
            if self.ctx.cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                return best_result;
            }

            self.stats.branches_expanded += 1;
            self.stats.max_depth_reached = self.stats.max_depth_reached.max(node.depth);

            // Termination policy check
            if self.policy.should_terminate(&self.stats, self.controller.as_ref()) {
                break;
            }

            // Check completeness — branch.is_complete() is a method on PlanBranch
            if node.branch.is_complete(self.ctx.request_tx) {
                // Forward validate the complete chain from real initial state
                let fwd_result = forward_validate(
                    &node.branch.action_chain,
                    &node.branch.action_bindings,
                    self.ctx,
                );
                if let Some((action_chain, total_cost)) = fwd_result {
                    if total_cost < best_cost {
                        best_cost = total_cost;
                        best_result = Some(PlanResult {
                            success: true,
                            action_chain,
                            total_cost,
                            goal_index: goal_index as i64,
                            deferred_action_indices: vec![],
                            action_bindings: node.branch.action_bindings.clone(),
                        });
                        self.policy.on_valid_plan_found(best_cost, &self.stats);
                        self.stats.best_cost = best_cost;
                        self.stats.valid_plans_found += 1;
                    }
                }
                continue;
            }

            // Prune by depth (matches current `depth > ctx.max_depth` semantics)
            if node.depth > self.ctx.max_depth {
                self.stats.branches_pruned += 1;
                continue;
            }

            // Heuristic-augmented cost pruning: g + h >= best_cost
            let f_score = node.branch.estimated_cost + node.estimated_remaining;
            if f_score >= best_cost {
                self.stats.branches_pruned += 1;
                continue;
            }

            // Expand and push successors
            let successors = BranchExpander { ctx: self.ctx }.expand(&node);
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
- **GoalSelection handles goal iteration** — which goals to try, in what order, whether to skip satisfied goals, and whether to short-circuit on first success.
- **Heuristic-augmented pruning** (`g + h >= best_cost`) prunes branches whose best-case outcome can't beat the current best plan. This is the key optimization that makes exhaustive search practical.
- **Cancel flag** is checked at the top of each loop iteration and between goals.
- **Tree dump** calls (not shown for brevity) are preserved at the same logical points: `enter_node` before expansion, `exit_node` after each branch outcome, `add_fwd_step` during forward validation.
- **`SearchContext` extension:** The current `SearchContext` does not have a `goals` field (goals are local variables in `run_plan`). For the new architecture, `SearchContext` gains `goals: &'a [GoalSpec]` so `GoalSelection::select_goals` and the engine's goal iteration loop can access them.

---

## 7. Per-Agent Configuration

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

enum GoalSelectionStrategy {
    HIGHEST_REWARD_FIRST = 0,
    HIGHEST_REWARD_FIRST_NO_SKIP = 1,
    ALL_GOALS_BEST_PLAN = 2,
    ALL_GOALS_BEST_PLAN_NO_SKIP = 3,
    BEST_REWARD_COST_RATIO = 4,
}

@export var search_strategy: SearchStrategy = SearchStrategy.DFS
@export var termination_strategy: TerminationStrategy = TerminationStrategy.FIRST_VALID
@export var goal_selection_strategy: GoalSelectionStrategy = GoalSelectionStrategy.HIGHEST_REWARD_FIRST
@export var max_goals_to_consider: int = 0         # 0 = all goals; N = only top N by reward
@export var max_search_time_ms: int = 50          # For TIME_BUDGET / BEST_WITHIN_BUDGET
@export var max_search_branches: int = 10000       # Hard cap on branches explored
@export var beam_width: int = 0                     # 0 = unlimited (for future Beam search)
```

### Bridge/Scheduler

The `SearchContext` gets a new `strategy_config` field. The `run_plan` function constructs the appropriate `Box<dyn SearchController>`, `Box<dyn TerminationPolicy>`, and `Box<dyn GoalSelection>` based on the config.

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

fn build_goal_selection(config: &StrategyConfig) -> Box<dyn GoalSelection> {
    let max_goals = config.max_goals_to_consider.max(0) as usize;
    match config.goal_selection_strategy {
        GoalSelectionStrategy::HighestRewardFirst => Box::new(HighestRewardFirst::new(max_goals)),
        GoalSelectionStrategy::HighestRewardFirstNoSkip => Box::new(HighestRewardFirstNoSkip::new(max_goals)),
        GoalSelectionStrategy::AllGoalsBestPlan => Box::new(AllGoalsBestPlan::new(max_goals)),
        GoalSelectionStrategy::AllGoalsBestPlanNoSkip => Box::new(AllGoalsBestPlanNoSkip::new(max_goals)),
        GoalSelectionStrategy::BestRewardCostRatio => Box::new(BestRewardCostRatio::new(max_goals)),
    }
}
```

---

## 8. Benchmarking

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

## 9. Implementation Phases

### Phase 1: Create `planner/` directory + Iterative DFS
**Goal:** Replace recursive `backward_search` with iterative loop, keep DFS behavior.
- Create `src/planner/` directory.
- Move candidate-finding, branch-cloning, and open-need-update logic into `planner/expander.rs` as `BranchExpander`.
- Implement `DfsController` in `planner/controller.rs` (Vec stack).
- Implement `FirstValidPolicy` and `ExhaustivePolicy` in `planner/policy.rs`.
- Implement `HighestRewardFirst` goal selection in `planner/goal_selection.rs` (current behavior).
- Implement `PlannerEngine` in `planner/engine.rs` with the iterative loop.
- Convert `planner.rs` to a shim that re-exports `PlannerEngine::run_plan` so existing callers in `scheduler.rs` compile unchanged.
- **Validation:** `make test-rust`, `make test-godot`. No behavior change expected.

### Phase 2: Add Heuristic-Augmented Pruning
**Goal:** Make exhaustive search practical by pruning branches whose best-case outcome can't beat the current best plan.
- In the main loop, before expanding a node, check `node.branch.estimated_cost + node.estimated_remaining >= best_cost`.
- Implement the admissible heuristic in `planner/heuristic.rs` (see [§ Resolved Questions](#-resolved-questions)).
- Note: basic cost-based pruning (`estimated_cost >= best_cost`) already exists in the current recursive code. This phase adds the heuristic `h(n)` term to make pruning tighter.
- This alone should make the current campfire scene with `ExhaustivePolicy` terminate in < 1 second instead of "endless".
- **Validation:** Interactive campfire test with `TerminationStrategy::Exhaustive`.

### Phase 3: Implement A* and Dijkstra Controllers + Goal Selection Variants
**Goal:** Add informed search strategies.
- Implement `AStarController` in `planner/controller.rs` with `BinaryHeap`.
- Design admissible heuristic in `planner/heuristic.rs` for `estimated_remaining`.
- Implement `DijkstraController` in `planner/controller.rs` (A* with h=0).
- **Validation:** Benchmark Phase 1 scenarios against new strategies. A* should find cheaper plans than DFS-first-valid with fewer branches.

### Phase 4: Implement Budget Policies + Remaining Goal Selection Strategies
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

## 10. Resolved Questions

1. **Heuristic for A*:** **Decision:** Use `count(open_preconditions) * min_action_cost + count(open_requirements) * min_provision_cost` as the admissible heuristic. Both preconditions and requirements are open needs that must be satisfied by predecessor actions. `min_action_cost` and `min_provision_cost` are the minimum costs observed across all actions in the catalog (cached once at search start). This is cheap to compute and guaranteed not to overestimate. We can improve it later if benchmarks show A* is exploring too many nodes.

2. **Visited-state deduplication:** **Decision:** Likely not possible in this domain because different branch chains (different action sequences) can produce the same open needs but with different accumulated costs and action bindings. However, we will investigate cost-based pruning first; if that is insufficient, we can experiment with a `HashSet<BranchFingerprint>` in `AStarController` as an optional optimization.

3. **Beam search worth it?** **Decision:** Defer. The modular architecture makes it trivial to add a `BeamController` later. Cost-based pruning (Phase 2) may already provide enough of the benefit.

4. **Probability-based pruning:** **Decision:** Do not implement at start. `forward_validate` remains the ground truth. We will rely on cost-based pruning and admissible heuristics rather than probabilistic estimates.

---

## 11. Files to Create / Modify

### New files
- `addons/GdPlanningAI/rust/src/planner/mod.rs` — module root, re-exports public API
- `addons/GdPlanningAI/rust/src/planner/engine.rs` — `PlannerEngine` + orchestration loop
- `addons/GdPlanningAI/rust/src/planner/expander.rs` — `BranchExpander`
- `addons/GdPlanningAI/rust/src/planner/controller.rs` — `SearchController` trait + `DfsController`, `AStarController`, `DijkstraController`
- `addons/GdPlanningAI/rust/src/planner/policy.rs` — `TerminationPolicy` trait + `FirstValidPolicy`, `ExhaustivePolicy`, `BudgetPolicy`, `BestWithinBudgetPolicy`
- `addons/GdPlanningAI/rust/src/planner/goal_selection.rs` — `GoalSelection` trait + `HighestRewardFirst`, `HighestRewardFirstNoSkip`, `AllGoalsBestPlan`, `AllGoalsBestPlanNoSkip`
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
- [ ] An agent config can select a search strategy, termination strategy, and goal selection strategy, and the planner respects all three.
- [ ] The planner does not feel "endless" in the campfire scene with `ExhaustivePolicy` (target: < 2 seconds for depth 6).
- [ ] `HighestRewardFirst` goal selection reproduces current behavior exactly (skip satisfied goals, first valid plan wins).
- [ ] `HighestRewardFirstNoSkip` attempts to plan even for already-satisfied goals.

---

## Next Step

Review this plan. If approved, we begin Phase 1: extracting `BranchExpander` and converting the recursive DFS to an iterative loop while preserving existing behavior.
