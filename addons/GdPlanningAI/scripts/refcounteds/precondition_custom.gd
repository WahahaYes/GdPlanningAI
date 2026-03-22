class_name PreconditionCustom
extends Precondition

var eval_func: Callable

func _init(fn: Callable) -> void:
	eval_func = fn

func _do_evaluate(agent: GdPAIBlackboard, world: GdPAIBlackboard) -> bool:
	return eval_func.call(agent, world)

