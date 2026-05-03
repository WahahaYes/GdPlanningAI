# Bug: `gdformat` duplicates file-level docstrings into multi-line lambda bodies inside dict literals

**gdformat version:** 4.5.0

## Summary

When a `.gd` file contains top-level `##` docstrings on functions and dict literals whose values are multi-line inline lambdas, `gdformat` incorrectly injects preceding file-level `##` docstring lines into the body of each multi-line lambda. This is **not idempotent** — running `gdformat` a second time duplicates the already-injected content again, growing the file unboundedly.

## Reproduction

**Input file:**

```gdscript
## A top-level docstring for this file.
extends Node


## Performs a simple action on the agent blackboard.
func do_something() -> void:
	var actions: Array[Dictionary] = [
		{
			"name": "MyAction",
			"cost_callable": func(_agent: Dictionary, _world: Dictionary) -> float:
				return 1.0,
			"effect_callable": func(agent: Dictionary, _world: Dictionary) -> void:
				var x = agent.get("value", 0)
				agent["value"] = x + 1,
		}
	]
	print(actions)
```

**Command:**
```sh
gdformat repro.gd
```

## Actual output after first run

```gdscript
## A top-level docstring for this file.
extends Node


## Performs a simple action on the agent blackboard.
func do_something() -> void:
	var actions: Array[Dictionary] = [
		{
			"name": "MyAction",
			"cost_callable": func(_agent: Dictionary, _world: Dictionary) -> float: return 1.0,
			"effect_callable":
			func(agent: Dictionary, _world: Dictionary) -> void:
## A top-level docstring for this file.

## Performs a simple action on the agent blackboard.

				var x = agent.get("value", 0)
				agent["value"] = x + 1,
		}
	]
	print(actions)
```

The file-level `##` docstrings were injected into the body of the multi-line lambda. Each lambda receives all docstrings that appear *before* it in the file.

## Second run makes it worse

Running `gdformat repro.gd` again on the already-corrupted output re-injects the (now larger) docstring block again, growing the file further. When I caught this bug while testing out `gdformat` in my pre-commit hook, I had a few files grow to thousands of lines.

## Observations

- The bug only triggers when the lambda body spans **multiple lines**. Single-line lambdas (where `gdformat` collapses them to `func(...): return x`) are not affected.
- The injection point is always immediately after the `func(...) -> T:` line of the lambda, before the first real body statement.
- This suggests the formatter's internal state for "pending docstrings to attach to the next function definition" is not being scoped correctly to top-level definitions, and is instead consuming all `##` blocks when it encounters any `func` keyword — including inline lambdas inside dict literals.

## Expected behaviour

`gdformat` should not modify comment/docstring placement inside dict-literal values. Inline `func` lambdas used as dict values should not be treated as top-level function definitions for the purposes of docstring attachment.
