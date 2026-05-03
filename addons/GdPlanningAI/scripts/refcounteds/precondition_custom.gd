class_name PreconditionCustom
extends Precondition
## Custom precondition that evaluates using a user-defined callable.
##
## This allows for arbitrary logic in precondition evaluation by accepting a callable
## that receives the agent and world blackboards and returns a boolean result.
## The callable is invoked on the main thread via the background planning callback system.
##
## Example:
## [codeblock]
## var precond = PreconditionCustom.new(
##     func(agent: GdPAIBlackboard, world: GdPAIBlackboard) -> bool:
##         return agent.get_property("health") > 50
## )
## [/codeblock]
##
## For preconditions that depend on specific scene objects, consider using
## [PreconditionCustomWithDeps] instead to enable dependency tracking.

## The callable that evaluates this precondition. Should accept two parameters
## (agent blackboard, world blackboard) and return a boolean.
var eval_func: Callable


## Creates a new custom precondition with the specified evaluation callable.
##
## @param fn A callable that takes (GdPAIBlackboard, GdPAIBlackboard) and returns bool
func _init(fn: Callable) -> void:
	eval_func = fn


## Evaluates the custom callable against the given blackboard states.
## This method is bound as the [code]eval_callable[/code] when serializing
## to the Rust bridge via [method to_bridge_dict].
func _do_evaluate(agent: GdPAIBlackboard, world: GdPAIBlackboard) -> bool:
	return eval_func.call(agent, world)


## Serializes this precondition into the dictionary format expected by the Rust bridge.
func to_bridge_dict() -> Dictionary:
	var eval: Callable = func(a: GdPAIBlackboard, w: GdPAIBlackboard) -> bool:
		return _do_evaluate(a, w)
	return {
		"operation": "custom_callback",
		"eval_callable": eval,
	}
