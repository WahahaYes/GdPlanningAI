# Provision/Action Index Design

## Problem

`find_candidates` in `expander.rs` performs a full O(actions × needs) scan on every node expansion:

```rust
for (idx, action) in ctx.actions.iter().enumerate() {
    // 1. Run validity checks against initial state
    // 2. Match provisions against ALL open requirements
    // 3. Simulate action and check effects against ALL open preconditions
}
```

In the campfire scene with many identical spatial objects, this means every `GoToAction` is re-evaluated against every open need on every expansion, even when no `at_target` requirement is open.

## Goal

Replace the full scan with targeted lookups so `find_candidates` only considers actions whose provisions *could* satisfy one of the branch's open needs.

## Design

### 1. Index Structure

Add two fields to `SearchContext`, built once when the context is created in `scheduler.rs`:

```rust
pub struct SearchContext {
    // ... existing fields ...

    /// Maps a requirement pattern to action indices that *might* satisfy it.
    /// Key: (provision_kind, name) where name is binding_name or fact_name.
    /// Value: Vec of action indices whose provisions match the key.
    pub provision_index: HashMap<(ProvisionKind, String), Vec<usize>>,

    /// Action indices that have NO wildcards (can be discovered once with empty bindings).
    pub non_wildcard_actions: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ProvisionKind {
    Binding,
    Fact,
    FactWildcard,
}
```

**Index construction** (`scheduler.rs`, after `action_specs` are built):

```rust
let mut provision_index: HashMap<(ProvisionKind, String), Vec<usize>> = HashMap::new();
let mut non_wildcard_actions = Vec::new();

for (idx, action) in action_specs.iter().enumerate() {
    let has_wildcard = action.provisions.iter().any(|p| matches!(p, ProvisionSpec::FactWildcard { .. }));
    if !has_wildcard {
        non_wildcard_actions.push(idx);
    }

    for prov in &action.provisions {
        let (kind, name) = match prov {
            ProvisionSpec::Binding { binding_name, .. } => (ProvisionKind::Binding, binding_name.clone()),
            ProvisionSpec::Fact { fact_name, .. } => (ProvisionKind::Fact, fact_name.clone()),
            ProvisionSpec::FactWildcard { fact_name } => (ProvisionKind::FactWildcard, fact_name.clone()),
        };
        provision_index.entry((kind, name)).or_default().push(idx);
    }
}
```

### 2. Indexed `find_candidates`

**For requirements** (the sequential bottleneck):

Only `pos == 0` requirements matter. For each such requirement, look up matching action indices via the index instead of scanning all actions:

```rust
let mut candidate_actions: HashSet<usize> = HashSet::new();

// 1. Requirement-driven lookup (sequential bottleneck)
for (_, (pos, req)) in branch.open_requirements.iter().enumerate() {
    if *pos != 0 { continue; }

    let lookup_key = match req {
        RequirementSpec::BindingExists { binding_name } |
        RequirementSpec::BindingEquals { binding_name, .. } |
        RequirementSpec::BindingInSet { binding_name, .. } => {
            (ProvisionKind::Binding, binding_name.clone())
        }
        RequirementSpec::Fact { fact_name, .. } => {
            (ProvisionKind::Fact, fact_name.clone())
        }
    };

    if let Some(action_indices) = ctx.provision_index.get(&lookup_key) {
        candidate_actions.extend(action_indices);
    }
    // Also check FactWildcard matches for Fact requirements
    if matches!(req, RequirementSpec::Fact { .. }) {
        let wildcard_key = (ProvisionKind::FactWildcard, lookup_key.1.clone());
        if let Some(action_indices) = ctx.provision_index.get(&wildcard_key) {
            candidate_actions.extend(action_indices);
        }
    }
}
```

**For preconditions** (non-sequential):

Preconditions can be satisfied by *any* action's effect, not just provisions. So we need to consider:
1. Actions already in the candidate set from requirement lookups (they might also satisfy preconditions)
2. Non-wildcard actions whose effects haven't been discovered yet — we still need to simulate them to check if they satisfy preconditions

Actually, preconditions are satisfied by **simulated effects**, not provisions. The index only helps with the *provision → requirement* matching. For preconditions, we still need to run `get_discovery_result` on candidate actions and then check if their effects satisfy open preconditions.

But the key win is: **we don't simulate actions that have no chance of satisfying any requirement at pos==0**.

For actions without wildcards, we can batch-discover their effects upfront (they use empty bindings). So the `non_wildcard_actions` list lets us pre-discover effects for simple actions.

**Revised flow:**

```rust
// 1. Collect candidate action indices from index lookups
let mut candidate_actions = HashSet::new();

// From requirements at pos==0
for (_, (pos, req)) in branch.open_requirements.iter().enumerate() {
    if *pos != 0 { continue; }
    lookup_requirement(ctx, req, &mut candidate_actions);
}

// From preconditions: we can't index preconditions directly since effects
// are dynamic. But if there are no open requirements, we still need to
// consider non-wildcard actions that might satisfy preconditions.
if branch.open_requirements.is_empty() && !branch.open_preconditions.is_empty() {
    candidate_actions.extend(&ctx.non_wildcard_actions);
}

// 2. Evaluate each candidate action (validity + discovery + precondition check)
for idx in candidate_actions {
    let action = &ctx.actions[idx];
    // ... existing validity + discovery + precond logic, but only for
    // actions that made it into the candidate set
}
```

### 3. Precondition Index (Optional Second Phase)

For an even bigger win, we could also index action *effects* by which precondition types they tend to satisfy. But effects are dynamic (depend on bindings), so this is harder. A simpler approach:

- Cache `DiscoveryResult` per action per binding set (already done)
- Precompute `non_wildcard` discovery results once at context creation time
- For preconditions, only simulate actions whose *known* cached effects match at least one open precondition

### 4. Edge Cases

- **No open requirements, only open preconditions**: Fall back to `non_wildcard_actions` list. Wildcard actions still need full evaluation since their bindings affect effects.
- **All requirements already satisfied**: `candidate_actions` is empty; use `non_wildcard_actions` for precondition-only search.
- **Wildcard actions**: The index already includes them under `FactWildcard`. Each open `Fact` requirement triggers both a `Fact` lookup and a `FactWildcard` lookup.
- **Actions with multiple provisions**: The index maps each provision independently. An action with both `held_item` and `at_target` provisions will appear in both index entries.

## Expected Impact

In the campfire scene:
- **Before**: Every expansion scans all ~10 actions against all open needs
- **After**: When `at_target(tree3)` is the only pos==0 requirement, only `GoToAction` appears in the candidate set. `Eat`, `PickUp`, `AddFuel`, etc. are skipped entirely.

**Complexity change:**
- Before: `O(actions × needs)` per expansion
- After: `O(needs × avg_providers_per_need + discovery_cost_for_candidates)`

For scenes with many identical objects (many `GoTo` variants), this changes the dominant term from `actions` to `needs`, which is typically much smaller.

## Implementation Order

1. Add `ProvisionKind` enum and `provision_index`/`non_wildcard_actions` fields to `SearchContext`
2. Build the index in `scheduler.rs` after `action_specs` are deserialized
3. Rewrite `find_candidates` to use the index for requirement-driven lookup
4. Add the `non_wildcard_actions` fallback for precondition-only expansions
5. Run tests to verify no regressions
6. Benchmark campfire integration tests for timeout reduction

## Files to Modify

- `planner/types.rs` — Add `ProvisionKind` enum, add index fields to `SearchContext`
- `scheduler.rs` — Build index after action deserialization
- `planner/expander.rs` — Rewrite `find_candidates` to use indexed lookup
- Integration tests — Verify campfire timeouts improve or stay consistent
