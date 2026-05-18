# Hybrid Backward-Chaining Planner: Implementation Pseudocode

## 1. Data Structures

### PlanBranch
- `action_chain`: Ordered list of action indices: `[A, B, C]`
- `final_state`: The deep-simulated state after the last action in the chain (`C`).
- `open_preconditions`: Accumulated physical needs from **anywhere** in the chain that are not yet met by `InitialState`. (The **Frontier**). A branch is only complete when this is empty — preconditions from multiple chain positions may coexist here.
- `open_requirements`: Symbolic needs (Potato, Axe) from **anywhere** in the chain.
  - Stored as `(consumer_pos, RequirementSpec)`.
- `action_bindings`: List of `(pos, key, values: Vec<VariantSnapshot>)` to be passed to Godot during simulation. Supports strings, integers, and object references.
- `cost`: Total cumulative cost from the latest forward simulation ripple.

### ActionCandidate
- `action_idx`: Index of the action being considered for prepending.
- `estimated_cost`: 1.0 (Physical) or 1.1 (Symbolic) to prioritize direct grounding.
- `satisfied_precondition_indices`: Which items in the current Frontier this action resolves.
- `satisfied_requirement_indices`: Which symbolic needs this action provides for.

---

## 2. Core Search Loop (A*)

1. **Initialization**:
   - Sort `Goals` by `Reward` (descending). Ignore goals with reward <= 0.
   - For each `Goal`:
     - If `Goal` is satisfied in `InitialState` (Deep Check):
       - RETURN successful empty plan for this goal.
     - Else:
       - `root = new PlanBranch`
       - `root.open_preconditions = unmet Goal.preconditions`
       - `frontier.push(root)`
       - Run A* Loop (see below). If success, RETURN plan.

2. **A* Loop**:
   - `visited = HashSet<NodeFingerprint>`
   - While `frontier` not empty:
     - `node = frontier.pop()`
     - If `visited.contains(node.fingerprint)`: CONTINUE.
     - `visited.insert(node.fingerprint)`
     - If `node.branch.is_complete()`: RETURN `branch.action_chain`.
     - `candidates = find_candidates(node.branch)`
     - For `candidate` in `candidates`:
       - `new_branch = expand(node.branch, candidate)`
       - If `new_branch` exists: `frontier.push(new_branch)`

---

## 3. Expansion Logic (`expand`)

When prepending `Action P` to `Branch [A, B, C]`:

1. **Prepend Action**: `new_chain = [P, A, B, C]`.
2. **Update Bindings**:
   - Shift indices of all existing `action_bindings` by +1.
   - For each symbolic `req` in `branch.open_requirements` satisfied by `P`:
     - Extract `values` from `P.provisions`.
     - Add new binding: `(new_consumer_pos, key, values)`.
3. **Full Forward Simulation (The Ripple)**:
   - `current_state = InitialState`
   - `current_provisions = InitialProvisions`
   - For `(pos, action)` in `new_chain`:
     - **Head Simulation**: If `pos == 0`:
       - `skip_validity = true`. (Head of chain is allowed to be ungrounded).
       - Note: The action's `simulate_effect` should report its potential effects even if requirements are not yet physically met in `current_state`.
     - Else:
       - `skip_validity = false`. (Remaining chain must be strictly grounded).
     - `res = action.simulate_effect(current_state, current_provisions, bindings[pos], skip_validity)`
     - If `res` is invalid: RETURN None (Prune).
     - `current_state = res.state`, `total_cost += res.cost`.
     - Update `current_provisions` with `action.provisions` for the NEXT step.
4. **Update Needs**:
   - **Initial State Pre-binding**:
     - Check `P.requirements` against `InitialProvisions`.
     - If met, extract bindings immediately and do NOT add to `open_requirements`.
   - **Requirements**:
     - Remove `open_requirements` satisfied by `P`.
     - Shift `consumer_pos` of remaining requirements by +1.
     - Add remaining `P.requirements` at `pos=0`.
   - **Preconditions (The Frontier)**:
     - Remove from `open_preconditions` only the entries that `P` satisfies (from `satisfied_precondition_indices`).
     - Check `P.preconditions` against `InitialState` (Deep Check).
     - Any NOT met are **appended** to `open_preconditions`.
     - Note: Unsatisfied preconditions from prior chain positions remain open until a future predecessor resolves them. The ripple validates that these are genuinely met when the full chain is executed.

---

## 4. Completion Check (`is_complete`)

A branch is complete if:
1. `open_preconditions` is empty (All accumulated physical needs across the chain are grounded in the Present).
2. `open_requirements` is empty (All symbolic dependencies are solved).
3. **Deep Goal Check**: The `final_state` satisfies all `Goal.preconditions`.

---

## 5. Candidate Discovery (`find_candidates`)

To find actions that *could* help, we use a two-layer check:

1. **Symbolic Layer**: 
   - Does `Action A` have a provision that matches **any** `open_requirement`?
   - **Context-Aware**: `BindingInSet` requirements check the world state for group membership.
   - Multiple satisfaction: An action can satisfy multiple open requirements at once.
2. **Optimistic Physical Layer**:
   - **Hypothetical State**: Create a state where concrete `BindingEquals` requirements are applied.
   - **Action-Led Hypothetical Progress**: Run `A.simulate_effect(HypotheticalState)` with **Strict=False**.
   - **Requirement Responsibility**: Actions that declare symbolic requirements (Existence, Set membership) should report their effects during simulation even if the requirement is not physically fulfilled in the snapshot.
   - If the effect satisfies **any** `open_precondition`, it is a candidate.

**Qualification rule**: An action qualifies as a candidate if it satisfies **at least one** open precondition OR **at least one** open requirement. It does NOT need to resolve all of them — that is the termination criterion (`is_complete`), not the candidate criterion.

**Cost Heuristic**:
- `est_cost = 1.0` if physical needs satisfied.
- `est_cost = 1.1` if only symbolic needs satisfied (encourages grounding).
