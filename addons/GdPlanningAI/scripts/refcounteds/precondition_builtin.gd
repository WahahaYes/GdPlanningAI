class_name PreconditionBuiltin
extends Precondition

enum Target { AGENT, WORLD_STATE }
enum Op { HAS_PROPERTY, EQUAL, NOT_EQUAL, GT, GTE, LT, LTE }

var target: Target
var operation: Op
var property: String
var value: Variant

func _init(t: Target, op: Op, prop: String, val: Variant = null) -> void:
	target = t
	operation = op
	property = prop
	value = val

func _do_evaluate(agent: GdPAIBlackboard, world: GdPAIBlackboard) -> bool:
	var source = agent if target == Target.AGENT else world
	match operation:
		Op.HAS_PROPERTY:
			return property in source.get_dict()
		Op.EQUAL:
			return source.get_property(property) == value
		Op.NOT_EQUAL:
			return source.get_property(property) != value
		Op.GT:
			return source.get_property(property) > value
		Op.GTE:
			return source.get_property(property) >= value
		Op.LT:
			return source.get_property(property) < value
		Op.LTE:
			return source.get_property(property) <= value
	return false

