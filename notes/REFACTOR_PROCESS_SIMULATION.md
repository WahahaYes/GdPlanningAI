# Refactor: Break `PlannerEngine::process_simulation` into Helpers

**Context:** `process_simulation` in `addons/GdPlanningAI/rust/src/planner/engine.rs` has grown into a single 250+ line method that handles the entire simulation/verification step. It is now responsible for initial-state requirement clearing, precondition re-evaluation, open-precondition evaluation, action simulation, requirement forward-validation, provision-based requirement clearing, and best-plan finalization. This makes the method hard to unit test and easy to regress when adding new validation logic.

**Goal:** split the method into smaller, stateless-ish helpers that each have a single responsibility, while preserving the current async `StepResult` flow and the "remove one precondition at a time and re-queue" safety pattern.

## Current responsibilities in `process_simulation`

1. **Initial-state requirement clearing** (only at `simulation_index == 0`).
2. **Gather current bindings** for the current chain position.
3. **Re-evaluate the action's own preconditions** against the current simulated state (built-ins only; custom ones skipped intentionally).
4. **Evaluate `open_preconditions`** at the current position, removing them one-by-one and returning `Ready` after each removal to stay async-safe.
5. **Simulate the current action** via `simulate_action`.
6. **Forward-validate action requirements** during `Verifying`.
7. **Concretize `FactWildcard` provisions** with the current chain-position binding and clear later `open_requirements`.
8. **Advance `simulation_index`** and update cost.
9. **Terminal verification** when the end of the chain is reached: ensure no open needs remain and update `best_plan`/`best_cost`.

## Proposed helper structure

Keep `process_simulation` as the thin orchestrator that delegates to helpers and decides which `StepResult` to return. Move the detailed logic into private methods on `PlannerEngine`.

```rust
impl PlannerEngine {
    fn process_simulation(&mut self, node: &mut SearchNode) -> StepResult<()> {
        let branch = &mut node.branch;

        // 1. Initial-state bookkeeping at the start of the chain.
        if branch.simulation_index == 0 {
            self.clear_initial_state_requirements(branch);
        }

        let current_bindings = Self::collect_bindings_for_position(
            &branch.action_bindings,
            branch.simulation_index,
        );

        // 2. Validate the action at the current position (if any).
        if branch.simulation_index < branch.action_chain.len() {
            let action_idx = branch.action_chain[branch.simulation_index];
            let action = &self.ctx.actions[action_idx];

            if let Some(result) = self.validate_action_against_current_state(
                branch,
                action,
                &current_bindings,
                node.callback_response.as_ref(),
            ) {
                return result;
            }

            if let Some(result) = self.evaluate_open_preconditions_for_position(
                branch,
                &current_bindings,
                node.callback_response.as_ref(),
            ) {
                return result;
            }

            return self.simulate_and_advance(
                branch,
                action_idx,
                action,
                &current_bindings,
                node.callback_response.as_ref(),
            );
        }

        // 3. End-of-chain handling.
        self.finalize_verified_branch(branch)
    }
}
```

Each helper returns `Option<StepResult<()>>` so the orchestrator can short-circuit on `Pending`, `Invalid`, or the intentional one-at-a-time `Ready` returns.

### Helper 1: `clear_initial_state_requirements`

Encapsulate the `simulation_index == 0` retain logic. This keeps the "initial state satisfies pos-0 requirements" rule in one place.

```rust
fn clear_initial_state_requirements(&self, branch: &mut PlanBranch) {
    branch.open_requirements.retain(|(pos, req)| {
        !(*pos == 0
            && self.ctx.initial_provisions.iter().any(|prov| {
                provision_satisfies_requirement(prov, req, Some(&self.ctx.initial_world))
            }))
    });
}
```

### Helper 2: `collect_bindings_for_position`

Pure function. Builds the `(name, values)` vector for the current position. This is trivial but extracting it makes the orchestrator read more clearly and makes binding collection independently testable.

```rust
fn collect_bindings_for_position(
    action_bindings: &[(usize, String, Vec<VariantSnapshot>)],
    position: usize,
) -> Vec<(String, Vec<VariantSnapshot>)> {
    action_bindings
        .iter()
        .filter(|(pos, _, _)| *pos == position)
        .map(|(_, name, vals)| (name.clone(), vals.clone()))
        .collect()
}
```

### Helper 3: `validate_action_against_current_state`

Re-evaluates the action's own preconditions (built-ins only) against the current simulated state. Returns `Some(StepResult)` if the caller should short-circuit: `Invalid` during `Verifying` on a false built-in, `Pending` on a custom/async precondition, or `Invalid` for any hard failure.

```rust
fn validate_action_against_current_state(
    &self,
    branch: &PlanBranch,
    action: &ActionSpec,
    current_bindings: &[(String, Vec<VariantSnapshot>)],
    callback_response: Option<&CallbackResponse>,
) -> Option<StepResult<()>> {
    let open_pre_for_current: HashSet<PreconditionSpec> = branch
        .open_preconditions
        .iter()
        .filter(|(pos, _)| *pos == branch.simulation_index)
        .map(|(_, pre)| pre.clone())
        .collect();

    for pre in &action.preconditions {
        if open_pre_for_current.contains(pre) || matches!(pre, PreconditionSpec::Custom { .. }) {
            continue;
        }
        match eval_precondition(
            pre,
            &branch.current_agent,
            &branch.current_world,
            &self.ctx,
            callback_response,
            current_bindings,
        ) {
            StepResult::Ready(true) => {}
            StepResult::Ready(false) => {
                if branch.state == BranchState::Verifying {
                    return Some(StepResult::Invalid);
                }
            }
            StepResult::Pending(id) => return Some(StepResult::Pending(id)),
            StepResult::Invalid => return Some(StepResult::Invalid),
            StepResult::Complete => unreachable!("eval_precondition cannot return Complete"),
        }
    }
    None
}
```

### Helper 4: `evaluate_open_preconditions_for_position`

Iterates `branch.open_preconditions` and evaluates entries at the current position. Removes the first satisfied one and returns `Some(StepResult::Ready(()))` to force a re-queue. This preserves the existing one-at-a-time async-safety behavior.

```rust
fn evaluate_open_preconditions_for_position(
    &self,
    branch: &mut PlanBranch,
    current_bindings: &[(String, Vec<VariantSnapshot>)],
    callback_response: Option<&CallbackResponse>,
) -> Option<StepResult<()>> {
    let mut i = 0;
    while i < branch.open_preconditions.len() {
        if branch.open_preconditions[i].0 == branch.simulation_index {
            match eval_precondition(
                &branch.open_preconditions[i].1,
                &branch.current_agent,
                &branch.current_world,
                &self.ctx,
                callback_response,
                current_bindings,
            ) {
                StepResult::Ready(true) => {
                    branch.open_preconditions.remove(i);
                    return Some(StepResult::Ready(()));
                }
                StepResult::Ready(false) => {
                    if branch.state == BranchState::Verifying {
                        return Some(StepResult::Invalid);
                    }
                }
                StepResult::Pending(id) => return Some(StepResult::Pending(id)),
                StepResult::Invalid => return Some(StepResult::Invalid),
                StepResult::Complete => unreachable!("eval_precondition cannot return Complete"),
            }
        }
        i += 1;
    }
    None
}
```

### Helper 5: `simulate_and_advance`

Calls `simulate_action`, validates requirements if in `Verifying`, concretizes wildcard provisions, clears satisfied later requirements, advances `simulation_index`, and recalculates cost. This is the largest remaining helper, but it is still smaller than today's full method and has one clear purpose: "execute this step and move forward."

```rust
fn simulate_and_advance(
    &self,
    branch: &mut PlanBranch,
    action_idx: usize,
    action: &ActionSpec,
    current_bindings: &[(String, Vec<VariantSnapshot>)],
    callback_response: Option<&CallbackResponse>,
) -> StepResult<()> {
    log_debug!("process_simulation sim_idx={} action={} ...", branch.simulation_index, action.name);

    if branch.state == BranchState::Verifying {
        for req in &action.requirements {
            if !requirement_holds_in_state(
                req,
                &branch.current_agent,
                &branch.current_world,
                current_bindings,
            ) {
                log_debug!("... requirement failed ...");
                return StepResult::Invalid;
            }
        }
    }

    match simulate_action(
        action_idx,
        SimArgs {
            agent: &branch.current_agent,
            world: &branch.current_world,
            ctx: &self.ctx,
            response: callback_response,
            branch_action_costs: &mut branch.action_costs,
            simulation_index: branch.simulation_index,
            bindings: current_bindings,
        },
    ) {
        StepResult::Ready(res) => {
            branch.current_agent = res.agent;
            branch.current_world = res.world;
            self.clear_requirements_from_provisions(
                branch,
                action_idx,
                current_bindings,
            );
            branch.simulation_index += 1;
            branch.recalculate_cost();
            StepResult::Ready(())
        }
        StepResult::Pending(id) => StepResult::Pending(id),
        StepResult::Invalid => StepResult::Invalid,
        StepResult::Complete => unreachable!("simulate_action cannot return Complete"),
    }
}
```

### Helper 6: `clear_requirements_from_provisions`

Extracts the wildcard-concretization and `open_requirements.retain` logic into a dedicated helper. This is the piece most likely to need future tweaks (e.g. more precise position ranges), so isolating it is valuable.

```rust
fn clear_requirements_from_provisions(
    &self,
    branch: &mut PlanBranch,
    action_idx: usize,
    current_bindings: &[(String, Vec<VariantSnapshot>)],
) {
    let action = &self.ctx.actions[action_idx];
    let concrete_provisions: Vec<ProvisionSpec> = action
        .provisions
        .iter()
        .map(|prov| concretize_wildcard_provision(prov, current_bindings))
        .collect();

    for prov in &concrete_provisions {
        branch.open_requirements.retain(|(pos, req)| {
            !(*pos >= branch.simulation_index
                && provision_satisfies_requirement(prov, req, Some(&branch.current_world)))
        });
    }
}
```

A small pure helper `concretize_wildcard_provision` can live alongside it in the module.

### Helper 7: `finalize_verified_branch`

Handles the end-of-chain terminal state. Checks for remaining open needs, updates the search tree, and records `best_plan` if the cost improved.

```rust
fn finalize_verified_branch(&mut self, branch: &mut PlanBranch) -> StepResult<()> {
    match branch.state {
        BranchState::Verifying => {
            if !branch.open_preconditions.is_empty() {
                self.tree.set_outcome(
                    branch.tree_node_id,
                    NodeOutcome::Pruned {
                        reason: "Unsatisfied preconditions remain".to_string(),
                    },
                );
                return StepResult::Invalid;
            }
            if !branch.open_requirements.is_empty() {
                self.tree.set_outcome(
                    branch.tree_node_id,
                    NodeOutcome::Pruned {
                        reason: "Unsatisfied requirements remain".to_string(),
                    },
                );
                return StepResult::Invalid;
            }
            if branch.cost < self.best_cost {
                self.tree.set_outcome(
                    branch.tree_node_id,
                    NodeOutcome::Complete {
                        chain_len: branch.action_chain.len(),
                        total_cost: branch.cost,
                        fwd_ok: true,
                    },
                );
                self.best_cost = branch.cost;
                self.best_plan = Some(/* PlanResult from branch fields */);
            }
            StepResult::Complete
        }
        BranchState::Searching => StepResult::Ready(()),
    }
}
```

## Ordering constraints to preserve

- `clear_initial_state_requirements` must run before any requirement checks at `simulation_index == 0`.
- `validate_action_against_current_state` must run before `evaluate_open_preconditions_for_position` because it deals with preconditions that are *not* in the open list.
- `evaluate_open_preconditions_for_position` intentionally returns early after removing one precondition. This is a safety mechanism for async callbacks; keep it.
- `simulate_and_advance` must run only after all preconditions for the current position are satisfied.
- `finalize_verified_branch` only runs when `simulation_index` has reached the end of the chain.

## Benefits

- Each helper is independently testable or at least readable in isolation.
- The async callback flow is explicit: only the orchestrator and the "evaluate" helpers return `Pending`/`Invalid`/`Ready`.
- Adding new validation rules (e.g. "re-check validity checks during simulation") becomes a matter of inserting another helper call, not extending a monolithic block.
- The debug logging can be attached to the relevant helper rather than scattered through one large method.

## Risks / things to verify during the refactor

- Ensure custom preconditions are still skipped in the re-evaluation helper and handled only via `open_preconditions`.
- Preserve the exact `StepResult` semantics for the state machine in `step_search` (especially `Ready` vs `Complete` at end-of-chain).
- The `branch_action_costs` mutability in `simulate_action` must stay visible; do not accidentally clone it away.
