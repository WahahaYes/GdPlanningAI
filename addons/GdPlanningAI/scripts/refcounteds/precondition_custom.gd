# SPDX-License-Identifier: Apache-2.0
# Copyright 2025 Ethan Wilson
# https://github.com/WahahaYes/GdPlanningAI

class_name PreconditionCustom
extends Precondition
## Precondition backed by a [Callable]; create it with [method Precondition.custom].
## @experimental: Internal to GdPlanningAI; may change in any release.

## Takes (agent blackboard, world blackboard) and returns a [bool].
var eval_func: Callable


func _init(fn: Callable) -> void:
	eval_func = fn


## Calls [member eval_func]; bound into [method to_bridge_dict].
func _do_evaluate(agent: GdPAIBlackboard, world: GdPAIBlackboard) -> bool:
	return eval_func.call(agent, world)


func to_bridge_dict() -> Dictionary:
	var eval: Callable = func(a: GdPAIBlackboard, w: GdPAIBlackboard) -> bool:
		return _do_evaluate(a, w)
	return {
		"operation": "custom_callback",
		"eval_callable": eval,
	}
