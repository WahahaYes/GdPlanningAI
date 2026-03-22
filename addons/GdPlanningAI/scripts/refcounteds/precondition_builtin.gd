class_name PreconditionBuiltin
extends Precondition

enum Target {AGENT, WORLD_STATE}
enum Op {HAS_PROPERTY, EQUAL, NOT_EQUAL, GT, GTE, LT, LTE}

var target: Target
var operation: Op
var property: String
var value: Variant


func _init(t: Target, op: Op, prop: String, val: Variant = null) -> void:
	target = t
	operation = op
	property = prop
	value = val


## Serializes this precondition into the dictionary format expected by the Rust bridge.
func to_bridge_dict() -> Dictionary:
	return {
		"target": _target_to_string(target),
		"operation": _operation_to_string(operation),
		"property_name": property,
		"value": value,
	}


static func _target_to_string(t: Target) -> String:
	match t:
		Target.AGENT:
			return "agent"
		Target.WORLD_STATE:
			return "world_state"
		_:
			return "agent"


static func _operation_to_string(op: Op) -> String:
	match op:
		Op.HAS_PROPERTY:
			return "has_property"
		Op.EQUAL:
			return "equal"
		Op.NOT_EQUAL:
			return "not_equal"
		Op.GT:
			return "greater_than"
		Op.GTE:
			return "greater_than_or_equal"
		Op.LT:
			return "less_than"
		Op.LTE:
			return "less_than_or_equal"
		_:
			return "has_property"
