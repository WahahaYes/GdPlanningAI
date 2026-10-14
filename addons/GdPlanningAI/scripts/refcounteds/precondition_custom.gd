class_name PreconditionCustom
extends Precondition
## Precondition backed by a user-defined [Callable]. Usually created with [method Precondition.custom].[br]
##[br]
## The callable receives the simulated agent and world [GdPAIBlackboard]s and must return a
## [bool] (anything else counts as [code]false[/code]). The planning engine runs on a background
## thread but invokes the callable on the main thread, through the plan scheduler's callback
## queue, so it is opaque to the search and slower than a built-in precondition. It must not use
## [code]await[/code] or the scene tree.
## [codeblock]
## var precond: Precondition = Precondition.custom(
##     func(agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> bool:
##         return agent.get_property("health") > 50
## )
## [/codeblock]
## For conditions that capture specific scene objects, use [PreconditionCustomWithDeps] instead.

## The callable that evaluates this precondition. It takes two parameters (agent blackboard,
## world blackboard) and returns a [bool].
var eval_func: Callable


## Creates a custom precondition. [param fn] takes (agent blackboard, world blackboard) and
## returns a [bool].
func _init(fn: Callable) -> void:
	eval_func = fn


## Calls [member eval_func] with [param agent] and [param world]. Bound as the
## [code]eval_callable[/code] in [method to_bridge_dict]; the engine reaches it through the
## scheduler callback queue.
func _do_evaluate(agent: GdPAIBlackboard, world: GdPAIBlackboard) -> bool:
	return eval_func.call(agent, world)


## Serialises this precondition into the dictionary format expected by the Rust bridge.
func to_bridge_dict() -> Dictionary:
	var eval: Callable = func(a: GdPAIBlackboard, w: GdPAIBlackboard) -> bool:
		return _do_evaluate(a, w)
	return {
		"operation": "custom_callback",
		"eval_callable": eval,
	}
