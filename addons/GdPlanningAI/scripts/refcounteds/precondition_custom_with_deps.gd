class_name PreconditionCustomWithDeps
extends Precondition
## Custom callable precondition that also tracks the objects its callable captures. Usually created
## with [method Precondition.custom_with_deps].[br]
##[br]
## Before the callable runs, every object in [member dependent_objects] is checked with
## [code]is_instance_valid[/code]; if any is freed the precondition is [code]false[/code] and the
## callable is not invoked. The objects' instance ids are also passed to the engine as metadata.
## Same rules as [PreconditionCustom] otherwise: the callable runs on
## the main thread during planning, must return a [bool], and must not use [code]await[/code] or
## the scene tree.

## The callable that evaluates this precondition: (agent blackboard, world blackboard) -> [bool].
var eval_func: Callable
## Objects the callable depends on. A freed object makes the precondition [code]false[/code].
var dependent_objects: Array[Object] = []


## Creates the precondition from [param fn] and the objects it depends on in [param deps].
func _init(fn: Callable, deps: Array[Object] = []) -> void:
	eval_func = fn
	dependent_objects = deps


## Returns [code]false[/code] if any dependent object is freed, otherwise the result of
## [member eval_func]. Bound as the [code]eval_callable[/code] in [method to_bridge_dict].
func _do_evaluate(agent: GdPAIBlackboard, world: GdPAIBlackboard) -> bool:
	# Validate all dependent objects still exist before invoking
	for obj in dependent_objects:
		if not is_instance_valid(obj):
			return false

	return eval_func.call(agent, world)


## Serialises this precondition into the dictionary format expected by the Rust bridge.
## Includes [code]dependent_object_ids[/code] for dependency validity checking.
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
