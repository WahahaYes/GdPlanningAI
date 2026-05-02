class_name PreconditionCustomWithDeps
extends Precondition
## A custom callable precondition with explicit object dependency tracking.
## This allows the planner to validate that dependent objects still exist before
## invoking the precondition, preventing errors from freed objects.

var eval_func: Callable
var dependent_objects: Array[Object] = []


func _init(fn: Callable, deps: Array[Object] = []) -> void:
	eval_func = fn
	dependent_objects = deps


## Evaluates the custom callable against the given blackboard states.
## This method is bound as the [code]eval_callable[/code] when serializing
## to the Rust bridge via [method to_bridge_dict].
## Validates dependent objects before invoking to prevent lambda capture errors.


func _do_evaluate(agent: GdPAIBlackboard, world: GdPAIBlackboard) -> bool:
	# Validate all dependent objects still exist before invoking
	for obj in dependent_objects:
		if not is_instance_valid(obj):
			return false

	return eval_func.call(agent, world)


## Serializes this precondition into the dictionary format expected by the Rust bridge.
## Includes [code]dependent_object_ids[/code] for dependency validity checking.


func to_bridge_dict() -> Dictionary:
	var dep_ids: Array[int] = []
	for obj in dependent_objects:
		if is_instance_valid(obj):
			dep_ids.append(obj.get_instance_id())

	return {
		"operation": "custom_callback",
		"eval_callable":
		func(a: GdPAIBlackboard, w: GdPAIBlackboard) -> bool: return _do_evaluate(a, w),
		"dependent_object_ids": dep_ids,
	}
