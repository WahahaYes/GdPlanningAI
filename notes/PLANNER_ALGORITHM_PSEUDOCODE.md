# Hybrid Backward-Chaining Planner: Simplified Pseudocode

## 1. Data Structures

### PlanBranch
- `action_chain`: List of actions in execution order: `[A, B, C]`
- `final_state`: The state after the last action in the chain (C).
- `open_preconditions`: Physical needs of the **first** action (`A`) that are not yet met by the `InitialState`. This is the **Frontier** of the plan.
- `open_requirements`: Symbolic needs (Potato, Axe) from **anywhere** in the chain that have not yet been satisfied by a Provision.
- `bound_provisions`: List of all provisions provided by actions in the current chain.
- `cost`: Total cumulative cost from the latest forward simulation.

### ActionCandidate
- `action`: The action being considered for prepending.
- `satisfied_preconditions`: Indices of `open_preconditions` this action satisfies.
- `satisfied_requirements`: Indices of `open_requirements` this action satisfies.

---

## 2. Core Search Loop (A*)

1. **Initialization**:
   - `root = new PlanBranch`
   - `root.open_preconditions = Goal.preconditions`
   - `root.final_state = InitialState`
   - `frontier.push(root)`

2. **Loop**:
   - `branch = frontier.pop()`
   - If `branch.is_complete()`: RETURN `branch.action_chain`
   - `candidates = find_candidates(branch)`
   - For `candidate` in `candidates`:
     - `new_branch = expand(branch, candidate)`
     - If `new_branch` exists: `frontier.push(new_branch)`

---

## 3. Expansion Logic (`expand`)

When prepending `Action P` to `Branch [A, B, C]`:

1. **Prepend Action**: `new_chain = [P, A, B, C]`
2. **Immediate Grounding (P against InitialState)**:
   - `res_p = P.simulate_effect(InitialState)`
   - If `res_p` is invalid: RETURN None (Prune).
3. **Verify Progress (The Chaining Step)**:
   - **Preconditions**: Did `P` make progress on any of the current `open_preconditions`?
   - **Requirements**: Does `P` provide any of the current `open_requirements`?
   - If `P` satisfied **NOTHING**: RETURN None (Prune).
4. **Full Forward Simulation (The Ripple)**:
   - `current_state = res_p.state`
   - `total_cost = res_p.cost`
   - For `action` in `[A, B, C]`:
     - `res = action.simulate_effect(current_state)`
     - If `res` is invalid: RETURN None (Prune).
     - `current_state = res.state`
     - `total_cost += res.cost`
5. **Update Needs**:
   - `new_branch = branch.clone()`
   - `new_branch.action_chain = new_chain`
   - `new_branch.final_state = current_state`
   - `new_branch.cost = total_cost`
   - **Update Requirements**:
     - Remove `open_requirements` satisfied by `P`.
     - Add `P.requirements` to `open_requirements`.
   - **Update Preconditions (The Frontier)**:
     - Clear the old `open_preconditions` (P is now responsible for them).
     - Check `P.preconditions` against `InitialState`.
     - Any that are NOT met by `InitialState` become the **new** `open_preconditions`.

---

## 4. Completion Check (`is_complete`)

A branch is complete if:
1. `open_preconditions` is empty (The plan's first step is grounded in the Present).
2. `open_requirements` is empty (All symbolic dependencies are solved).
3. **Deep Goal Check**: The `final_state` satisfies the `Goal.preconditions`.

---

## 5. Candidate Discovery (`find_candidates`)

### The "Optimistic Discovery" Rule
To find actions that *could* help, we use a two-layer check:

1. **Symbolic Layer**: 
   - Does `Action A` have a provision that matches an `open_requirement`?
2. **Optimistic Physical Layer**:
   - Does `Action A` affect the properties mentioned in any `open_precondition`?
   - **Rule**: Run `A.simulate_effect(InitialState)` with **Strict=False** (ignore unmet requirements).
   - If the simulation reports progress on the target property, it is a candidate.
   - *Note: The "Forward Ripple" in Step 3 will later prove if this optimism was justified.*

