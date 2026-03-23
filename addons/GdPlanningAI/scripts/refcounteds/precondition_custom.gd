class_name PreconditionCustom
extends Precondition

## A custom callable that takes [param agent] and [param world] blackboards and returns a boolean.
var eval_func: Callable


func _init(fn: Callable) -> void:
	eval_func = fn


## Evaluates the custom callable against the given blackboard states.
## This method is bound as the [code]eval_callable[/code] when serializing
## to the Rust bridge via [method to_bridge_dict].
func _do_evaluate(agent: GdPAIBlackboard, world: GdPAIBlackboard) -> bool:
	return eval_func.call(agent, world)


## Serializes this precondition into the dictionary format expected by the Rust bridge.
func to_bridge_dict() -> Dictionary:
	return {
		"operation": "custom_callback",
		"eval_callable": func(a: GdPAIBlackboard, w: GdPAIBlackboard) -> bool:
			return _do_evaluate(a, w),
	}