class_name PreconditionBuiltin
extends Precondition
## Property comparison evaluated natively by the planning engine. Usually created through the
## static constructors on [Precondition] (for example [method Precondition.agent_property_equal_to])
## rather than directly.[br]
##[br]
## It compares one property (existence, equality or ordering) on the agent blackboard, on the world
## blackboard, or on world objects of a group, where the group form is true if [i]any[/i] object
## in the group satisfies it. Because the engine understands which property is tested, it can
## evaluate the check on its own thread without callbacks and can search for actions whose
## simulated effects change that property.

## Specifies which blackboard to evaluate the precondition against.
enum Target {
	AGENT,  ## The agent's blackboard
	WORLD_STATE,  ## The world state blackboard
	WORLD_OBJECT_PROXY,  ## Properties on world objects in a specific group
}

## Comparison operations supported by builtin preconditions.
enum Op {
	HAS_PROPERTY,  ## Check if a property exists
	EQUAL,  ## Check if a property equals a value
	NOT_EQUAL,  ## Check if a property does not equal a value
	GT,  ## Check if a property is greater than a value
	GTE,  ## Check if a property is greater than or equal to a value
	LT,  ## Check if a property is less than a value
	LTE,  ## Check if a property is less than or equal to a value
}

## Which blackboard to target (agent, world state, or world object proxy).
var target: Target
## The comparison operation to perform.
var operation: Op
## The name of the property to check.
var property: String
## For WORLD_OBJECT_PROXY: the group name to filter objects.
var group: String
## The value to compare against (not used for HAS_PROPERTY operation).
var value: Variant


## Creates a built-in precondition.[br]
##[br]
## [param t] is the target blackboard, [param op] the comparison, [param prop] the property name,
## [param val] the value to compare against (ignored for [constant Op.HAS_PROPERTY]) and
## [param grp] the object group (only used with [constant Target.WORLD_OBJECT_PROXY]).
func _init(t: Target, op: Op, prop: String, val: Variant = null, grp: String = "") -> void:
	target = t
	operation = op
	property = prop
	group = grp
	value = val


## Serialises this precondition into the dictionary format expected by the Rust bridge.
func to_bridge_dict() -> Dictionary:
	var dict = {
		"target": _target_to_string(target),
		"operation": _operation_to_string(operation),
		"property_name": property,
		"value": value,
	}
	if target == Target.WORLD_OBJECT_PROXY:
		dict["group"] = group
		dict["property"] = property
	return dict


## Converts a Target enum value to its string representation for the Rust bridge.
static func _target_to_string(t: Target) -> String:
	match t:
		Target.AGENT:
			return "agent"
		Target.WORLD_STATE:
			return "world_state"
		Target.WORLD_OBJECT_PROXY:
			return "world_object_proxy"
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
