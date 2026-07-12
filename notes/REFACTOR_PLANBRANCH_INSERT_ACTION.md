# Refactor: Encapsulate Expansion Insertion in `PlanBranch::insert_action_at`

**Context:** the expansion logic in `PlannerEngine::step_search` (around `addons/GdPlanningAI/rust/src/planner/engine.rs:381-528`) manually performs the insertion of a newly discovered predecessor action. It computes `insert_pos`, inserts into `action_chain` and `action_costs`, shifts all positions, clears requirements, adds new open needs, and records provider/consumer bindings. This is a lot of position-sensitive bookkeeping done inline in the engine, which makes the expansion hard to follow and easy to break when the insertion semantics change again.

**Goal:** move the entire insertion mutation into a `PlanBranch` method so the engine only has to decide *which* action to insert and *where*; the branch itself guarantees the invariants of the position fields.

## What the current inline expansion does

In order:

1. Compute `insert_pos` as the minimum position among all preconditions/requirements the candidate satisfies.
2. Fallback to `0` if the chain is empty.
3. Insert `action_idx` and `discovery_cost` into `action_chain`/`action_costs` at `insert_pos`.
4. Shift all positions in `open_preconditions`, `open_requirements`, and `action_bindings` that are `>= insert_pos` by `+1`.
5. For each satisfied requirement:
   - Greedily find all identical requirements in `open_requirements` and mark them for removal.
   - Compute the binding name/value from the provision/requirement pair.
   - Add a provider binding at `insert_pos`.
   - Add a consumer binding at each cleared consumer's shifted position.
6. Remove the satisfied requirements and preconditions.
7. Add the new action's own preconditions and requirements at `insert_pos`, skipping those already satisfied by the initial state or already present in the open lists.
8. Merge the new bindings into `action_bindings`.

## Proposed `PlanBranch` API

Add a single method on `PlanBranch` that performs the whole insertion atomically:

```rust
impl PlanBranch {
    /// Insert a predecessor action into the chain immediately before its consumers.
    ///
    /// * `insert_pos` is the position of the first consumer this action satisfies. The new action
    ///   is inserted at `insert_pos`, and all existing entries at or after `insert_pos` are shifted
    ///   forward by one.
    /// * `action_idx` is the index of the action in [`SearchContext::actions`].
    /// * `cost` is the discovered cost of the action.
    /// * `satisfied_requirements` lists the requirements this action satisfies, with their
    ///   positions in `open_requirements` **before** the shift and the provision that matched them.
    /// * `satisfied_preconditions` lists the indices into `open_preconditions` **before** the shift
    ///   that this action satisfies.
    /// * `new_preconditions` and `new_requirements` are the action's own unsatisfied needs that
    ///   should be added at the new action's position, after the shift.
    ///
    /// Returns the new bindings that were created so the caller can log them or attach them to a
    /// debug tree. The branch is left in `Searching` state; the caller decides whether to start
    /// verification.
    pub fn insert_action_at(
        &mut self,
        insert_pos: usize,
        action_idx: usize,
        cost: f64,
        satisfied_requirements: Vec<(usize, RequirementSpec, ProvisionSpec)>,
        satisfied_precondition_indices: HashSet<usize>,
        new_preconditions: Vec<PreconditionSpec>,
        new_requirements: Vec<RequirementSpec>,
    ) -> Vec<(usize, String, Vec<VariantSnapshot>)> {
        // 1. Insert the action and cost.
        self.action_chain.insert(insert_pos, action_idx);
        self.action_costs.insert(insert_pos, cost);

        // 2. Shift all position-tracked entries at or after insert_pos.
        self.shift_positions(insert_pos, 1);

        // 3. Collect requirements to remove and build bindings.
        let mut reqs_to_remove: HashSet<usize> = HashSet::new();
        let mut new_bindings = Vec::new();

        for (req_idx, req, prov) in satisfied_requirements {
            // Greedy clearing: all identical requirements are satisfied by this one action.
            for (idx, (_, other_req)) in self.open_requirements.iter().enumerate() {
                if other_req == &req {
                    reqs_to_remove.insert(idx);
                }
            }

            let (binding_name, values) = binding_values_from_match(&prov, &req);
            if !binding_name.is_empty() {
                // Provider binding: at the newly inserted action's position.
                new_bindings.push((insert_pos, binding_name.clone(), values.clone()));

                // Consumer bindings: at each shifted consumer position.
                for &idx in &reqs_to_remove {
                    let consumer_pos = self.open_requirements[idx].0;
                    new_bindings.push((consumer_pos, binding_name.clone(), values.clone()));
                }
            }
        }

        // 4. Remove satisfied needs.
        remove_indices(&mut self.open_requirements, &reqs_to_remove);
        remove_indices(&mut self.open_preconditions, &satisfied_precondition_indices);

        // 5. Add the new action's own needs at the new action's position.
        for pre in new_preconditions {
            self.open_preconditions.push((insert_pos, pre));
        }
        for req in new_requirements {
            self.open_requirements.push((insert_pos, req));
        }

        // 6. Merge new bindings.
        self.action_bindings.extend(new_bindings.clone());

        new_bindings
    }
}
```

A small pure helper extracts the binding name/value from a provision/requirement pair, which is currently duplicated inline:

```rust
fn binding_values_from_match(
    prov: &ProvisionSpec,
    req: &RequirementSpec,
) -> (String, Vec<VariantSnapshot>) {
    match (prov, req) {
        (
            ProvisionSpec::Binding {
                binding_name,
                value,
            },
            _,
        ) => (binding_name.clone(), vec![value.clone()]),
        (ProvisionSpec::Fact { fact_name, args }, _) => (fact_name.clone(), args.clone()),
        (
            ProvisionSpec::FactWildcard { fact_name },
            RequirementSpec::Fact { args, .. },
        ) => (fact_name.clone(), args.clone()),
        _ => (String::new(), vec![]),
    }
}
```

## What the engine expansion block would look like

With the helper in place, the engine code shrinks to the decision-making parts:

```rust
// Inside the find_candidates / expansion loop:
let mut new_branch = node.branch.clone();
let action = &self.ctx.actions[cand.action_idx];

let insert_pos = compute_insert_pos(
    &new_branch,
    &cand.satisfied_preconditions,
    &cand.satisfied_requirements,
);

let discovery_cost = {
    let cache = self.ctx.discovery_results.lock().unwrap();
    cache
        .get(&(cand.action_idx, cand.bindings.clone()))
        .map(|r| r.cost)
        .unwrap_or(1.0)
};

let new_preconditions: Vec<PreconditionSpec> = action
    .preconditions
    .iter()
    .filter(|pre| {
        !new_branch.open_preconditions.iter().any(|(_, p)| p == *pre)
            && pre.evaluate_builtin(&self.ctx.initial_agent, &self.ctx.initial_world)
                .unwrap_or(false)
                == false
    })
    .cloned()
    .collect();

let new_requirements: Vec<RequirementSpec> = action
    .requirements
    .iter()
    .filter(|req| {
        !new_branch.open_requirements.iter().any(|(_, r)| r == *req)
            && !self.ctx.initial_provisions.iter().any(|prov| {
                provision_satisfies_requirement(prov, req, Some(&self.ctx.initial_world))
            })
    })
    .cloned()
    .collect();

let new_bindings = new_branch.insert_action_at(
    insert_pos,
    cand.action_idx,
    discovery_cost,
    cand.satisfied_requirements,
    cand.satisfied_preconditions.iter().copied().collect(),
    new_preconditions,
    new_requirements,
);

// Tree/debugging work still happens in the engine because it needs SearchContext/TreeDump.
let satisfied_pre: Vec<String> = cand
    .satisfied_preconditions
    .iter()
    .map(|&idx| node.branch.open_preconditions[idx].1.to_string())
    .collect();
let satisfied_req: Vec<String> = cand
    .satisfied_requirements
    .iter()
    .map(|(_, req, _)| req.to_string())
    .collect();

new_branch.tree_node_id = self.tree.add_child(
    node.branch.tree_node_id,
    &action.name,
    discovery_cost,
    node.branch.cost + discovery_cost,
    &new_branch.open_preconditions.iter().map(|(_, p)| p.to_string()).collect::<Vec<_>>(),
    &new_branch.open_requirements.iter().map(|(_, r)| r.to_string()).collect::<Vec<_>>(),
    &satisfied_pre,
    &satisfied_req,
);

new_branch.state = BranchState::Searching;
new_branch.recalculate_cost();

if new_branch.open_preconditions.is_empty() && new_branch.open_requirements.is_empty() {
    new_branch.state = BranchState::Verifying;
    new_branch.simulation_index = 0;
    new_branch.current_agent = self.ctx.initial_agent.clone();
    new_branch.current_world = self.ctx.initial_world.clone();
}

if new_branch.open_requirements.len() > self.max_depth {
    self.tree.set_outcome(
        new_branch.tree_node_id,
        NodeOutcome::Pruned {
            reason: "Too many accumulated open requirements (cycle)".to_string(),
        },
    );
    continue;
}

self.enqueue(SearchNode {
    branch: new_branch,
    resumed: false,
    callback_response: None,
    expanded_candidates: Vec::new(),
});
```

The inline position arithmetic, the `shift_positions` call, the greedy requirement clearing, and the binding recording all move into `PlanBranch`.

## Where to put `compute_insert_pos`

`compute_insert_pos` can be a free function in `planner::types` or a private method on `PlanBranch`:

```rust
fn compute_insert_pos(
    branch: &PlanBranch,
    satisfied_precondition_indices: &[usize],
    satisfied_requirements: &[(usize, RequirementSpec, ProvisionSpec)],
) -> usize {
    let old_chain_len = branch.action_chain.len();
    let mut insert_pos = old_chain_len;

    for &idx in satisfied_precondition_indices {
        let pos = branch.open_preconditions[idx].0;
        if pos < insert_pos {
            insert_pos = pos;
        }
    }
    for (idx, _, _) in satisfied_requirements {
        let pos = branch.open_requirements[*idx].0;
        if pos < insert_pos {
            insert_pos = pos;
        }
    }

    if insert_pos == old_chain_len {
        insert_pos = 0;
    }
    insert_pos
}
```

This keeps the engine focused on search policy rather than data-structure mechanics.

## Edge cases the helper must handle

- **Empty chain:** `insert_pos` falls back to `0`; the chain and cost vectors become length 1.
- **Insert at the end:** when inserting at the old length, `shift_positions` should be a no-op for existing entries (they are all `< old_len`), but the new action sits at the end.
- **Multiple consumers of the same binding:** the greedy requirement clearing must collect all identical requirements before shifting positions, then use the *shifted* positions for consumer bindings. The current implementation does this; the helper must preserve the order: collect indices, shift, then read consumer positions from the shifted list.
- **Provider-only bindings:** if the provision does not produce a binding name (the fallback `_ => (String::new(), vec![])` case), no binding is recorded.
- **Duplicate needs:** the engine already deduplicates before calling the helper; the helper should not silently deduplicate. It should insert exactly what it is given, preserving the caller's policy.

## Benefits

- `PlanBranch` becomes the single source of truth for how actions, costs, positions, and bindings relate to each other.
- The engine no longer needs to know the exact `shift_positions` contract or the binding recording rules.
- Unit tests can create a `PlanBranch`, call `insert_action_at`, and assert on the resulting chain, positions, and bindings without spinning up a full `PlannerEngine`.
- Future changes to insertion semantics (e.g. inserting a *range* of actions at once, or supporting parallel branches) only require changes in one place.

## Longer-term vision: from vector chain to dependency graph

The current `action_chain: Vec<usize>` is a linearization of what is fundamentally a partial order: action A must happen before action B if B consumes a precondition/requirement provided by A. The position-shifting machinery exists because the planner stores the plan as a linear sequence and then renumbers everything on every insertion.

A more radical refactor would store the plan as a directed acyclic graph (DAG) of action nodes plus explicit dependency edges. Each node would record its own requirements and provisions; an action is executable once all its incoming dependencies are satisfied. The final plan would be a topological sort of the DAG, with ties broken by cost. This would eliminate the `insert_pos` and `shift_positions` machinery entirely, but it would require rethinking:

- The debug tree formatting, which currently assumes a linear chain.
- The `simulate_action` order, which also assumes a linear chain.
- The heuristic priority, which currently operates on a `PlanBranch` cost.
- How bindings are scoped: in a DAG, a binding would be associated with an edge or a node rather than a chain position.

This is a much larger project and should be treated as a separate architectural spike, not part of the immediate `insert_action_at` refactor.

## Suggested implementation order

1. Add `binding_values_from_match` and `compute_insert_pos` as private helpers in `planner::types` or `planner::engine`.
2. Implement `PlanBranch::insert_action_at` and add focused unit tests in `addons/GdPlanningAI/rust/tests/` that exercise:
   - Inserting the first action into an empty branch.
   - Inserting before a consumer at position 2 and verifying positions shift correctly.
   - Greedy clearing of multiple identical requirements.
   - Recording both provider and consumer bindings.
3. Replace the inline expansion block in `engine.rs` with a call to `insert_action_at`.
4. Run `make test-rust`, `make build-release`, and `make test-godot` to ensure no regressions.
