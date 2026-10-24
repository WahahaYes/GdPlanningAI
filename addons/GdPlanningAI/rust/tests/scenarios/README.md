# Planner scenarios

Each `<slug>.toml` holds the cases proving one behavior in [docs/BEHAVIORS.md](../../../../../docs/BEHAVIORS.md). `tests/scenarios.rs` runs every case through `PlannerEngine::new` with a scripted host and reports it as `<slug>::<name>`. Run one file with `cargo test --test scenarios -- <slug>`. Adding a case needs no Rust.

```toml
[[case]]
name = "pickup_then_eat"                 # unique in the file
max_depth = 10                           # optional; default 100
first_complete = true                    # optional; default BestCost
agent = { hunger = 80 }                  # initial agent properties
world = { time = 12.5 }                  # initial world properties
objects = [{ uid = "101", groups = ["food"], properties = { hunger_value = 30 } }]
agent_objects = [{ uid = "me", groups = ["GdPAILocationData"], properties = { position = 0 } }]
provisions = [{ binding = "held_item", value = { object = 101 } }]   # hold before any action

[[case.actions]]
name = "eat"
cost = 5.0                               # omitted: 1.0 with no callback
effect = { set = { fed = true }, add = { hunger = -60 } }           # omitted: no effect
preconditions = [{ agent = "has_food" }]
validity = [{ world = "open", op = "==", value = true }]
requires = [{ binding_exists = "held_item" }]
provides = [{ fact = "is_fed", args = [] }]

[[case.goals]]                           # in priority order (highest reward first)
name = "satisfy_hunger"
reward = 10.0
want = [{ agent = "hunger", op = "<", value = 30 }]

[case.expect]
plan = ["pickup", "eat"]                 # action names in order; or fails = true
cost = 10.0                              # optional
goal = "satisfy_hunger"                  # optional
bindings = [{ at = 0, name = "held_item", values = [{ object = 101 }] }]   # optional, must be present
```

- **Values:** ints, floats, bools, strings, and `{ object = <id> }` for object references.
- **Conditions:** one of `agent = "<prop>"`, `world = "<prop>"` or `group = "<group>", prop = "<prop>"`, with `op` (`has`, the default, or `==`, `!=`, `>`, `>=`, `<`, `<=`) and `value`.
- **Custom conditions:** `{ custom = <answer> }`, where the host answers with a constant (`true`/`false`), `{ binding = "<name>", value = ... }` (the planner injected that binding value), or a condition on the state it is asked about.
- **Requirements:** `binding_exists`, `binding_equals` (+ `value`), `binding_in_set` (+ `set`), `fact` (+ `args`). **Provisions:** `binding` (+ `value`), `fact` (+ `args`), `fact_wildcard`.
- Effects only change the agent. Cases that count callbacks, time slices or log output stay as integration tests beside this harness.
