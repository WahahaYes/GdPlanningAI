class_name PreconditionCustomWithDeps
extends Precondition
## Custom precondition that is [code]false[/code] once any object its callable captures is freed.
## Create it with [method Precondition.custom_with_deps].

## Takes (agent blackboard, world blackboard) and returns a [bool].
var eval_func: Callable
## Objects the callable captures.
var dependent_objects: Array[Object] = []


func _init(fn: Callable, deps: Array[Object] = []) -> void:
	eval_func = fn
	dependent_objects = deps


## Returns [code]false[/code] if a dependency is freed, else [member eval_func]'s result.
func _do_evaluate(agent: GdPAIBlackboard, world: GdPAIBlackboard) -> bool:
	# Validate all dependent objects still exist before invoking
	for obj in dependent_objects:
		if not is_instance_valid(obj):
			return false

	return eval_func.call(agent, world)


func to_bridge_dict() -> Dictionary:
	var dep_ids: Array[int] = []
	for obj in dependent_objects:
		if is_instance_valid(obj):
			dep_ids.append(obj.get_instance_id())

	var eval: Callable = func(a: GdPAIBlackboard, w: GdPAIBlackboard) -> bool:
		return _do_evaluate(a, w)
	return {
		"operation": "custom_callback",
		"eval_callable": eval,
		"dependent_object_ids": dep_ids,
	}
