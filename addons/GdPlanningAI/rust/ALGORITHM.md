# Planning Algorithm

This document describes the step-by-step process the planner uses to formulate a plan. The planner uses **backward-chaining GOAP** (Goal-Oriented Action Planning): it starts from a desired goal state and works backward, selecting predecessor actions whose effects or provisions satisfy the open needs of the partial plan.

---

## Core Concepts

**Preconditions** — State conditions that must be true for an action to execute or a goal to be satisfied (e.g., "agent hunger > 50"). Evaluated against blackboard snapshots using built-in comparisons or custom GDScript callbacks.

**Validity Checks** — Like preconditions but used only during forward validation. They do not participate in backward chaining. They guard against runtime-inappropriate actions (e.g., "target object still exists").

**Requirements** — Symbolic dependencies an action declares it needs (e.g., "held_item must exist"). Satisfied by provisions from earlier actions.

**Provisions** — Symbolic contributions an action declares it supplies (e.g., "I provide held_item"). Consumed by later actions' requirements.

**Pending Effects** — When an action has unbound requirements, its effect cannot yet be concretely simulated to prove it satisfies an open precondition. A pending effect claim is recorded: a promise that the action will satisfy certain preconditions once its requirements are resolved by predecessor actions. Pending claims are re-simulated whenever new provisions become available.

**Hypothetical Snapshots** — Temporary state copies used during candidate classification. The planner injects bound provision values, simulates the action's effect, and checks whether open preconditions would become satisfied — without mutating the real accumulated state.

**Forward Validation** — The definitive verification. Once the backward search finds a complete chain, the entire chain is replayed from the **real initial state** in execution order. Every precondition, validity check, requirement, and cost is re-evaluated against the actual forward-accumulated state. The final state is checked against the goal. This prevents accepting plans built on incorrect hypothetical assumptions.

---

## Algorithm Steps

### Step 1: Goal Prioritization

All active goals are sorted by reward, highest first. Goals with equal reward keep their original registration order.

### Step 2: Early Satisfaction Check

The planner checks whether the goal is already satisfied in the current state. All of the goal's desired-state preconditions are evaluated against the initial agent and world snapshots. If every precondition passes, an empty plan (zero actions, zero cost) is returned immediately.

### Step 3: Initial Provision Extraction

The planner scans the agent's blackboard properties. Every non-null, non-empty property becomes a binding provision. These represent facts already true in the initial state that can satisfy requirements without an action.

### Step 4: Root Branch Creation

A root branch is created with:
- **Open preconditions:** the goal's desired-state preconditions.
- **Open requirements:** empty (goals do not declare requirements).
- **Action chain:** empty.
- **Bound provisions:** the initial provisions from Step 3.
- **Accumulated state:** the initial agent and world snapshots.

### Step 5: Recursive Backward Search

The core search is a depth-first recursive function. At each recursion level:

#### 5a. Pruning

Three conditions cause the branch to be abandoned:
1. **Cancellation:** A cancel flag is checked (set when a newer plan is submitted for the same agent).
2. **Cost pruning:** The branch's accumulated estimated cost already exceeds the best known valid plan cost.
3. **Depth limit:** The recursion depth exceeds the configured maximum.

#### 5b. Completion Check

The branch is complete when:
1. No pending effects remain unresolved.
2. Every open precondition is satisfied by the accumulated state.
3. Every open requirement is satisfied by the combined initial provisions and accumulated bound provisions (using context-aware matching that considers world object group memberships).

If complete, forward validation (Step 6) runs on the full chain. If it succeeds with a lower cost than the current best, this becomes the new best plan.

#### 5c. Candidate Discovery

Every available action is examined:

- **Validity gate:** Actions with freed dependent objects are excluded.
- **Requirement satisfaction:** If the branch has open requirements, the planner checks whether the action's provisions satisfy any. For wildcard provisions, cost is evaluated per matching requirement so the nearest/cheapest target can be selected.
- **Precondition satisfaction via effect:** A hypothetical snapshot is created from the accumulated state plus bound provisions. The action's effect is simulated, and preconditions that transition from unsatisfied to satisfied are identified.
- **Requirement-dependent effects:** If an action has requirements but its effect cannot currently prove satisfaction, the planner checks whether the effect *could* satisfy preconditions if all possible provisions from all actions were available. These become candidates flagged as requiring bound effects, creating pending effect claims.

Candidates are sorted by estimated cost (lowest first), with ties broken by action index.

#### 5d. Branch Expansion

For each candidate, in sorted order:

1. **Clone the branch.**
2. **Prepend the action** to the front of the chain (building backward).
3. **Add estimated cost** to the branch.
4. **Update open needs:**
   - Remove requirements satisfied by this action's provisions.
   - Capture wildcard bindings (action index, fact name, concrete object IDs).
   - Add the action's provisions to bound provisions.
   - Remove preconditions claimed by this action's effect.
   - If the action required bound effects, record a pending effect claim.
   - Add the action's own requirements as new open requirements (skip duplicates).
   - Add the action's own preconditions as new open preconditions, but only those not already satisfied by the accumulated state and not already in the open set.
   - Re-simulate all pending effect claims with the expanded bound provisions. Fully satisfied claims are cleared; partially satisfied ones are kept with remaining preconditions.
5. **Simulate effect forward** on the accumulated state, but only if the action's requirements are now fully met by bound provisions.
6. **Recurse** into the next depth level.
7. **Track the best result** across all candidates at this level.

### Step 6: Forward Validation

When a branch is declared complete, the action chain is validated from the real initial state. For each action in execution order:

1. **Dependency check:** Verify all dependent objects still exist.
2. **Validity checks:** Evaluate all validity-check preconditions.
3. **Preconditions:** Evaluate all action preconditions.
4. **Requirements:** Verify all requirements are satisfied by accumulated provisions (initial + prior actions).
5. **Cost:** Call the cost callable. If infinite, validation fails.
6. **Effect:** Call the effect callable, updating forward state.
7. **Provision accumulation:** Add provisions for subsequent actions.

After all actions, the goal's desired-state preconditions are evaluated against the final state. All must pass.

### Step 7: Result Selection

The search explores all candidate branches within the depth limit. The lowest-cost forward-validated plan is returned. If no plan is found for the highest-priority goal, the next goal is tried. If no goal can be satisfied, a failure result is returned.

---

## Requirement-Provision Matching

- **BindingExists:** Matches when a provision provides a binding with the same name and a non-null, non-empty value.
- **BindingEquals:** Matches when a provision provides a binding with the same name and exactly equal value.
- **BindingInSet:** Matches when a provision provides a binding with the same name and the value refers to a world object belonging to the specified group.
- **Fact:** Matches when a provision provides a fact with the same name and identical arguments.
- **FactWildcard:** Matches any fact requirement with a matching name, regardless of arguments. The concrete arguments from the requirement are captured as bindings for the providing action.

---

## Threading Model

The planner runs on a background thread (via Rayon). GDScript callables cannot be invoked from background threads, so all cost queries, effect simulations, and custom precondition evaluations are sent as callback requests through a channel to the main thread. The main thread drains these requests each frame via `process_callbacks` and delivers results back to the planner thread.
