# Planner Pseudocode vs. Implementation Audit

**Date:** 2026-06-14
**Scope:** `notes/reference/PLANNER_PSEUDOCODE.md` grounded against `addons/GdPlanningAI/rust/src/planner/`

## Executive Summary

The current Rust planner implements the *shape* of the hybrid backward-chaining algorithm described in the pseudocode, but there are three categories of drift:

1. **Algorithmic correctness bugs** in forward validation that can allow invalid plans to be accepted.
2. **Structural gaps** where the state machine described in the pseudocode is not fully realized (dead `Rippling` state, missing terminal precondition checks).
3. **Performance / search-control drift** where the pseudocode's A* / budget-yield semantics are approximated by a hard Dijkstra loop with a fixed iteration cap.

The planner is *feasible* as a backward-chaining planner, but the forward-validation path has a hole that could explain some of the lingering test/timeout issues.

---

## 1. Core Data Structures — Mostly Aligned

| Pseudocode | Implementation | Status |
|------------|----------------|--------|
| `PlanBranch` with `action_chain`, `action_costs`, `action_bindings`, `open_preconditions`, `open_requirements`, `state`, `simulation_index`, `cost` | `PlanBranch` in `planner/types.rs:25` has all these fields. | ✅ Aligned |
| `action_bindings: Vec<(pos, BindingData)>` | `action_bindings: Vec<(usize, String, Vec<VariantSnapshot>)>` — chain-position-scoped. | ✅ Aligned (and was explicitly migrated from action-index semantics in a prior fix). |
| `fingerprint: (goal_index, Vec<PreconditionSpec>, Vec<RequirementSpec>, state)` with positions stripped | `fingerprint()` in `planner/types.rs:162` does exactly this. | ✅ Aligned |
| `SearchNode { branch, resumed, callback_response }` | `SearchNode` in `planner/types.rs:54` has these fields. | ✅ Aligned |

---

## 2. Search Loop (`step_search`) — Partially Aligned, with Gaps

### 2.1 Phase 1: Resume Callbacks

**Pseudocode:**
- Drain `response_channel`.
- Update Discovery Cache.
- Find parked nodes, check cancel, push back to queue.

**Implementation** (`engine.rs:139-199`):
- Does drain `response_rx` and updates discovery caches. ✅
- Does revive parked nodes and push them back. ✅
- **Drift:** Cancellation immediately returns `Complete(None)` from `step_search` rather than just clearing parked nodes and continuing the loop. This is functionally close but aborts the entire planning step on the first callback after cancel, which is slightly different from the described flow.

### 2.2 Phase 2: A* Iteration

**Pseudocode:**
- Pop node.
- Visited check: if fingerprint seen and cost >= previous cost, **prune** (continue).
- Optimality check: if `node.cost >= best_cost`, **prune** (continue).

**Implementation** (`engine.rs:203-241`):
- Visited check is guarded by `!node.resumed && node.branch.state == BranchState::Searching`. This is an important optimization (resumed nodes haven't changed state), but the pseudocode presents the visited check as unconditional. Not a correctness issue, but a drift. ✅
- **Drift:** The optimality check at line 208-213 uses `node.priority() >= self.best_cost` and, if true, **pushes the node back onto the queue and breaks the loop entirely**. The pseudocode says "PRUNE" (discard/continue). Since `priority() == cost` (Dijkstra, h=0), if the best node in the min-heap is already worse than `best_cost`, all nodes are worse, so breaking is semantically acceptable. However, putting the node back on the queue means it will be re-popped next time and break again, which is harmless but untidy.

### 2.3 Phase 3: State Machine Processing

#### **State `Searching`** (Expansion)

**Pseudocode:**
1. If `open_preconditions` and `open_requirements` empty → `state = Verifying`, reset to initial state, push back.
2. Expand via `find_candidates`.
3. For each ready candidate:
   - Clone branch, prepend action.
   - Greedy clearing of matched and identical requirements.
   - Remove preconditions satisfied by discovery result.
   - Offset remaining needs by +1.
   - Add new action needs at pos 0.
   - Deduplicate via HashSet.
   - Stay in `Searching`.
4. Handle pending: park node.

**Implementation** (`engine.rs:270-514`):
- Completeness check and transition to `Verifying` are present and correct. ✅
- Candidate prepending, greedy clearing, offsetting, deduplication, and parking are all implemented. ✅
- **Extra behavior (not in pseudocode):** After adding new needs, the code checks if initial state satisfies new requirements and skips adding them if so (`engine.rs:424-436`). This is a valid optimization but not documented.
- **Extra behavior (not in pseudocode):** After creating the new branch, it does a post-hoc builtin precondition check against the discovery result and removes any pos-0 preconditions that are already satisfied (`engine.rs:471-491`). This is also a reasonable optimization but represents drift.

#### **State `Rippling` / `Verifying` (Forward Simulation)**

**Pseudocode:**
1. Only enter once symbolic needs resolved.
2. Evaluate `open_preconditions` where `pos == simulation_index`. **If false, return Invalid.** If true, remove.
3. Simulate action at `simulation_index`. Update snapshots. Accumulate provisions. Increment.
4. Terminal: if `simulation_index == chain.len()` and `Verifying`: if all needs empty, **Record Best Plan**.

**Implementation** (`engine.rs:563-741` via `process_simulation`):

**🚨 CRITICAL DRIFT 1: `Rippling` is a dead state.**
- `BranchState::Rippling` is defined in `types.rs` and matched in `engine.rs:244` and `engine.rs:663`, but **it is NEVER assigned anywhere in the codebase**.
- The implementation groups `Initializing | Rippling | Verifying` into the *same* `process_simulation` handler.
- The pseudocode describes `Rippling` as a distinct forward simulation phase. The implementation does not use it.

**🚨 CRITICAL DRIFT 2: Action preconditions that evaluate to `false` during forward validation are NOT treated as Invalid.**
- In `process_simulation` (`engine.rs:586-616`), when evaluating an open precondition at the current `simulation_index`:
  ```rust
  StepResult::Ready(false) => {
      // Not satisfied yet, keep it and check others at this index.
  }
  ```
- The code **does not return `Invalid`**; it keeps the precondition in the list and continues.
- The terminal check in `Verifying` (`engine.rs:686-698`) only checks for open preconditions at **`pos == simulation_index == chain.len()`**:
  ```rust
  let has_open_preconds = branch
      .open_preconditions
      .iter()
      .any(|(pos, _)| *pos == branch.simulation_index);
  ```
- Since `simulation_index == chain.len()` at the terminal point, this only checks for **goal preconditions** (which are at `pos == chain.len()` after all the prepends).
- **Action preconditions** live at positions `0 .. chain.len()-1`. If one evaluated to `false` during the forward pass, it remains in `open_preconditions` but at a position `< chain.len()`. The terminal check **never sees it**.
- **Conclusion:** A branch can pass forward validation even if an action precondition was false, as long as all goal preconditions are met. This is a **correctness bug**.

**🚨 CRITICAL DRIFT 3: `Rippling` terminal check only looks at goal preconditions.**
- In the `Rippling` match arm (`engine.rs:663-681`), after finishing a forward simulation pass, the code checks open preconditions **only at `pos == action_chain.len()`** and then transitions back to `Searching`.
- This confirms the same pattern: action-level preconditions are never terminal-checked.

**Correct behavior present:**
- Initial-state provision checking for requirements at `simulation_index == 0` ✅
- Current-bindings gathering for the simulation step ✅
- `simulate_action` call and snapshot update ✅
- Provision accumulation to clear downstream requirements ✅
- Terminal best-cost recording ✅

---

## 3. Candidate Discovery (`find_candidates`) — Structurally Aligned, Semantically Approximate

**Pseudocode:**
- Loop all actions.
- Check `validity_checks` against `InitialState`.
- Match Requirements (strict `pos == 0`).
- Match Preconditions (unrestricted any `pos`).
- Simulate Effect via Discovery Cache or callback.
- Return `CandidatesResult { ready_list, pending_id }`.

**Implementation** (`planner/expander.rs:129-421`):
- Iterates all actions, checks validity, matches requirements at `pos == 0`, matches preconditions at any position. ✅
- Discovery simulation uses cache or requests callback via `get_discovery_result`. ✅
- **Drift:** `get_discovery_result` always simulates from `ctx.initial_agent` / `ctx.initial_world` (`expander.rs:81-88`). For a branch that already has actions in its chain, when a *new* action is prepended at position 0, simulating from the real initial state is correct because that action will run first. However, for preconditions at higher positions (not the immediate next action), the candidate check evaluates them against the state **after only the new action**, ignoring the effects of any intermediate actions that would run between the new action and the precondition's owner. This can cause false-positive candidate matches for persistent-state preconditions (which is fine) but could mislead for state that gets changed by intermediate actions.
- **Drift:** There is no provision/action index. The expander does a full O(N_actions * N_needs) scan every time. The pseudocode doesn't require an index, but the performance notes in `BACKWARD_CHAINING_GOAP_PLANNER_PLAN.md` mention that provision/action indexes are still pending.

---

## 4. Async/Budget/Yield Semantics — Approximated

**Pseudocode:**
- Mentions `ITERATION_BUDGET` and yielding when exceeded.
- Describes `Pending(id)` parking and `Ready(value)` continuation.

**Implementation:**
- No `ITERATION_BUDGET` constant or configurable budget. Instead, a hardcoded `20000` iteration cap exists at `engine.rs:216`.
- When exceeded, it logs a warning, pushes the current node back, and returns `PlannerRunResult::Pending(0)`.
- The scheduler (`scheduler.rs`) uses a Rayon thread pool and a request/response channel pair. This is a working async architecture, but it is **synchronous blocking** inside `simulate_action` and `eval_precondition` (they block on `mpsc` recv or use timeout fallbacks). The pseudocode describes a truly non-blocking state-machine, but the implementation achieves non-blocking at the *search-loop* level by parking nodes, not at the *function-call* level with async/await.

---

## 5. Missing / Incomplete Search Controls

| Feature | Pseudocode / Plan | Implementation Status |
|---------|-------------------|----------------------|
| `ITERATION_BUDGET` | Mentioned | ❌ Hardcoded 20,000 cap only |
| A* heuristic (`h > 0`) | Described as A* wrapper | ❌ Dijkstra (`h = 0`) only; `priority() == cost` |
| `Rippling` state | Active forward simulation phase | ❌ Dead state — never assigned |
| Provision/action indexes | Recommended for performance | ❌ Not implemented; full scan every expansion |
| Visited state keyed by open needs + selected suffix | Recommended | ⚠️ Partial — fingerprint strips positions but does not include action chain |

---

## 6. Summary of Issues

### Category A: Algorithmic Incorrectness (must fix for correctness)
1. **Action preconditions false during forward validation do not invalidate the branch.** `process_simulation` must return `Invalid` when a precondition at the current `simulation_index` evaluates to `false`.
2. **Terminal validation in `Verifying` only checks goal preconditions.** It must also verify that *all* action preconditions were satisfied (i.e., `open_preconditions` is entirely empty, not just at `chain.len()`).

### Category B: Structural Drift (should fix for maintainability / performance)
3. **`Rippling` state is dead code.** Either implement it as a real intermediate forward-simulation state (as the pseudocode describes) or remove it and update the pseudocode.
4. **No configurable iteration budget.** The hardcoded 20,000 limit should be parameterized.
5. **No heuristic / A* implementation.** The `SearchAlgorithm` enum has `AStar` but `priority()` is pure Dijkstra. Either implement a heuristic or remove the enum variant.
6. **No provision/action indexes.** `find_candidates` scans all actions against all needs on every expansion. For large action sets this is the primary performance bottleneck.

### Category C: Working As Intended (minor drift, not bugs)
7. **Visited check skips resumed nodes.** This is an optimization, not a bug.
8. **Extra deduplication of initial-state-satisfied requirements.** This is a valid pruning optimization.
9. **Post-discovery builtin precondition clearing.** Also a valid optimization.

---

## 7. Recommendations

1. **Fix forward validation precondition handling.** In `process_simulation`, when `eval_precondition` returns `Ready(false)` for a precondition at the current `simulation_index`, return `StepResult::Invalid` immediately. Then update the terminal `Verifying` check to simply ensure `open_preconditions.is_empty()` (and same for requirements) rather than checking only at `simulation_index`.

2. **Decide the fate of `Rippling`.** Either:
   - Implement it properly: enter `Rippling` after `Searching` completes (all needs empty) and before `Verifying`; run one full forward simulation pass in `Rippling`, then transition to `Verifying` if all preconditions clear.
   - Or remove the state and update the pseudocode to describe the current two-state model (`Searching` → `Verifying`).

3. **Parameterize the iteration budget** and wire it through from `submit_plan` or the scheduler config.

4. **Consider a simple action-provision index** (e.g., a `HashMap<RequirementSpec, Vec<usize>>` of action indices) in `SearchContext` so `find_candidates` doesn't scan the entire action list on every node expansion.

5. **Re-run the full test suite** after fixing (1) and (2). The current timeout / search-explosion issues may be partially caused by invalid branches surviving forward validation and polluting the search tree or best-cost bound.

---

## Relevant File Citations

- `notes/reference/PLANNER_PSEUDOCODE.md` — Target specification
- `addons/GdPlanningAI/rust/src/planner/engine.rs:134-560` — `step_search` and `process_simulation`
- `addons/GdPlanningAI/rust/src/planner/engine.rs:586-616` — Precondition evaluation during forward simulation (bug location)
- `addons/GdPlanningAI/rust/src/planner/engine.rs:686-698` — Terminal goal-precondition-only check (bug location)
- `addons/GdPlanningAI/rust/src/planner/expander.rs:45-117` — `get_discovery_result`
- `addons/GdPlanningAI/rust/src/planner/expander.rs:129-421` — `find_candidates`
- `addons/GdPlanningAI/rust/src/planner/types.rs:12-18` — `BranchState` enum (includes unused `Rippling`)
- `addons/GdPlanningAI/rust/src/planner/types.rs:54-58` — `SearchNode`
- `addons/GdPlanningAI/rust/src/planner/types.rs:162-175` — `fingerprint()`
