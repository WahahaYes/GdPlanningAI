# A* Heuristic Causing Planner Timeout

**Date:** 2026-05-19

**Issue:**
Planner is timing out on complex scenarios (campfire, hunger examples) that previously worked with DFS. The A* search is getting stuck in infinite loops or exploring too many states without finding a valid plan within 300 frames.

**Symptoms:**
- test_full_cooking_chain times out waiting for plan
- test_hunger_example_smoke tests timeout
- Log shows extensive simulation activity without convergence
- 3-10 timeout failures depending on test run
- Log files 32k-90k lines (excessive simulation)

**Root Cause Analysis:**
The A* heuristic `estimate_remaining = (pre_count + req_count) * min_action_cost` is too weak and incorrectly biased. The user suspects that the relative cost of the heuristic is pushing actions with actual travel costs to the end of the queue, causing the planner to explore cheaper-but-wrong branches first.

**Current Heuristic:**
```rust
pub fn estimate_remaining(branch: &PlanBranch, min_action_cost: f64) -> f64 {
    let pre_count = branch.open_preconditions.len() as f64;
    let req_count = branch.open_requirements.len() as f64;
    (pre_count + req_count) * min_action_cost
}
```

**Problems:**
1. **Too weak**: Just counts open needs, doesn't account for:
   - One action satisfying multiple preconditions
   - Interactions between requirements
   - Actual costs of specific actions (especially travel costs)
   - Requirement dependencies

2. **Incorrect cost bias**: By using `min_action_cost`, the heuristic underestimates the cost of expensive actions (like GoTo with travel costs), causing the A* priority queue to prioritize these expensive actions later in the search.

**Test Case:**
Expected 5-action plan: GoTo(potato) → Dig Potato → GoTo(campfire) → Cook Potato → Eat Held Food
- GoTo actions have actual travel costs
- Current heuristic treats all actions as having equal cost (min_action_cost)
- This causes GoTo to be deprioritized in the queue

**Attempted Fixes:**
1. **Visited set fingerprint order-independence**: Attempted to use hash-based fingerprints instead of vector ordering. This made things worse (3→10 timeouts, 32k→90k log lines) due to hash collisions. Reverted.

**Next Steps:**
1. Improve heuristic to account for actual action costs from the action specs
2. Consider using average action cost instead of minimum
3. Add requirement dependency tracking to heuristic
4. Consider hybrid approach: A* with depth limiting or beam search
5. Consider reverting to DFS with better pruning if A* cannot be fixed

**Related Files:**
- `addons/GdPlanningAI/rust/src/planner/heuristic.rs` - Current heuristic implementation
- `addons/GdPlanningAI/rust/src/planner/controller.rs` - A* controller and visited set
- `addons/GdPlanningAI/rust/src/planner/engine.rs` - Search loop
