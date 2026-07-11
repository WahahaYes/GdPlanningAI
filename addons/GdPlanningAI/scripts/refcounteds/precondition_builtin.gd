class_name PreconditionBuiltin
extends Precondition
## Built-in precondition that evaluates property comparisons on blackboard states.
##
## This precondition type performs common operations like property existence checks,
## equality comparisons, and numeric comparisons. These operations are optimized
## and can be evaluated directly on the background thread without callbacks.

## Specifies which blackboard to evaluate the precondition against.
enum Target {
	AGENT, ## The agent's blackboard
	WORLD_STATE, ## The world state blackboard
}

## Comparison operations supported by builtin preconditions.
enum Op {
	HAS_PROPERTY, ## Check if a property exists
	EQUAL, ## Check if a property equals a value
	NOT_EQUAL, ## Check if a property does not equal a value
	GT, ## Check if a property is greater than a value
	GTE, ## Check if a property is greater than or equal to a value
	LT, ## Check if a property is less than a value
	LTE, ## Check if a property is less than or equal to a value
}

## Which blackboard to target (agent or world state).
var target: Target
## The comparison operation to perform.
var operation: Op
## The name of the property to check.
var property: String
## The value to compare against (not used for HAS_PROPERTY operation).
var value: Variant


## Creates a new builtin precondition with the specified parameters.
##
## @param t The target blackboard (AGENT or WORLD_STATE)
## @param op The comparison operation to perform
## @param prop The property name to check
## @param val The value to compare against (optional, not used for HAS_PROPERTY)
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


## Converts a Target enum value to its string representation for the Rust bridge.
static func _target_to_string(t: Target) -> String:
	match t:
		Target.AGENT:
			return "agent"
		Target.WORLD_STATE:
			return "world_state"
		_:
			return "agent"


## Converts an Op enum value to its string representation for the Rust bridge.
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
