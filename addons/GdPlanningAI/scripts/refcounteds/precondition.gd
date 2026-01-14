class_name Precondition
extends RefCounted
## Preconditions for planning chains of actions.  This class allows for custom preconditions
## to be defined and evaluation at simulation-time.  Stores a check ("is_satisfied") to keep
## track if this precondition has been satisfied at some other point in the chain of actions.
## For multithreaded planning, it is possible to await information with GdPAIUTILS.await_callv(..).
##[br]
##[br]
## There are static methods, like Precondition.agent_property_greater_than(<prop>, <value>),
## which will instantiate common preconditions in a single line of code.  Alterantively, custom
## preconditions can be defined like so:
##[br]
##[br]
## var precondition = Precondition.new()[br]
## precondition.eval_func = func(blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):[br]
##     if blackboard.get_property(<prop>) and world_state.get_property(<prop>):[br]
##         return true[br]
##     return false[br]
##[br]
##[br]
## (The above example would check that a property exists / is true for both the agent and world
## states.)

## Enum for specifying which blackboard to target for property checks.
enum Target {
	AGENT,
	WORLD_STATE,
}
## Enum for specifying the type of comparison operation.
enum Operation {
	HAS_PROPERTY,
	EQUAL,
	NOT_EQUAL,
	GREATER_THAN,
	GREATER_THAN_OR_EQUAL,
	LESS_THAN,
	LESS_THAN_OR_EQUAL,
}

## Variable to keep track of whether earlier evaluations of this precondition were successful.
var is_satisfied: bool = false
## The evaluation function is dynamic and can be set by instantiating the precondition using one
## of the static methods, or by defining a custom check.
var eval_func: Callable


## Instanciating a Precondition with a custom check. Must match function signature:
## [param func(blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard)->bool]
func _init(_eval_func: Callable) -> void:
	assert(_eval_func.is_valid(), "Evaluation function is not valid")
	
	# WARNING currently can't achieve type-safety on the passed _eval_func
	# If such a feature gets added, need to assert a signature of:
	# func(a: GdPAIBlackboard, b: GdPAIBlackboard) -> bool
	# See issue https://github.com/godotengine/godot-proposals/issues/10807
	
	eval_func = _eval_func


## Generic function to create property comparison preconditions, eliminating code duplication.
static func _create_property_precondition(
		target: Target,
		prop: String,
		operation: Operation,
		value: Variant = null,
) -> Precondition:
	return Precondition.new(
		func(
			blackboard: GdPAIBlackboard, 
			world_state: GdPAIBlackboard
		):
		var source: GdPAIBlackboard
		match target:
			Target.AGENT:
				source = blackboard
			Target.WORLD_STATE:
				source = world_state
			_:
				push_error("Unknown target: %s" % target)
				return null
		
		match operation:
			Operation.HAS_PROPERTY:
				return prop in source.get_dict()
			Operation.EQUAL:
				return source.get_property(prop) == value
			Operation.NOT_EQUAL:
				return source.get_property(prop) != value
			Operation.GREATER_THAN:
				return source.get_property(prop) > value
			Operation.GREATER_THAN_OR_EQUAL:
				return source.get_property(prop) >= value
			Operation.LESS_THAN:
				return source.get_property(prop) < value
			Operation.LESS_THAN_OR_EQUAL:
				return source.get_property(prop) <= value
			_:
				push_error("Unknown operation: %s" % operation)
				return null
	)


## Instantiate a precondition that checks whether a property in the agent blackboard exists.
static func agent_has_property(prop: String) -> Precondition:
	return _create_property_precondition(Target.AGENT, prop, Operation.HAS_PROPERTY)


## Instantiate a precondition that checks whether a property in the agent blackboard is not
## equal to a specified value.
static func agent_property_not_equal_to(
		prop: String,
		value: Variant,
) -> Precondition:
	return _create_property_precondition(Target.AGENT, prop, Operation.NOT_EQUAL, value)


## Instantiate a precondition that checks whether a property in the agent blackboard is greater
## than a specified value.
static func agent_property_greater_than(
		prop: String,
		value: Variant,
) -> Precondition:
	return _create_property_precondition(Target.AGENT, prop, Operation.GREATER_THAN, value)


## Instantiate a precondition that checks whether a property in the agent blackboard is greater
## or equal than a specified value.
static func agent_property_geq_than(
		prop: String,
		value: Variant,
) -> Precondition:
	return _create_property_precondition(Target.AGENT, prop, Operation.GREATER_THAN_OR_EQUAL, value)


## Instantiate a precondition that checks whether a property in the agent blackboard is less
## than a specified value.
static func agent_property_less_than(
		prop: String,
		value: Variant,
) -> Precondition:
	return _create_property_precondition(Target.AGENT, prop, Operation.LESS_THAN, value)


## Instantiate a precondition that checks whether a property in the agent blackboard is less
## than or equal to a specified value.
static func agent_property_leq_than(
		prop: String,
		value: Variant,
) -> Precondition:
	return _create_property_precondition(Target.AGENT, prop, Operation.LESS_THAN_OR_EQUAL, value)


## Instantiate a precondition that checks whether a property in the agent blackboard is equal to
## a specified value.
static func agent_property_equal_to(
		prop: String,
		value: Variant,
) -> Precondition:
	return _create_property_precondition(Target.AGENT, prop, Operation.EQUAL, value)


## Instantiate a precondition that checks whether a property in the agent blackboard exists.
static func world_state_has_property(prop: String) -> Precondition:
	return _create_property_precondition(Target.WORLD_STATE, prop, Operation.HAS_PROPERTY)


## Instantiate a precondition that checks whether a property in the world state is greater
## than a specified value.
static func world_state_property_greater_than(
		prop: String,
		value: Variant,
) -> Precondition:
	return _create_property_precondition(Target.WORLD_STATE, prop, Operation.GREATER_THAN, value)


## Instantiate a precondition that checks whether a property in the world state is greater
## or equal than a specified value.
static func world_state_property_geq_than(
		prop: String,
		value: Variant,
) -> Precondition:
	return _create_property_precondition(
		Target.WORLD_STATE,
		prop,
		Operation.GREATER_THAN_OR_EQUAL,
		value,
	)


## Instantiate a precondition that checks whether a property in the world state is less
## than a specified value.
static func world_state_property_less_than(
		prop: String,
		value: Variant,
) -> Precondition:
	return _create_property_precondition(Target.WORLD_STATE, prop, Operation.LESS_THAN, value)


## Instantiate a precondition that checks whether a property in the world state is less
## than or equal to a specified value.
static func world_state_property_leq_than(
		prop: String,
		value: Variant,
) -> Precondition:
	return _create_property_precondition(
		Target.WORLD_STATE,
		prop,
		Operation.LESS_THAN_OR_EQUAL,
		value,
	)


## Instantiate a precondition that checks whether a property in the world state is equal to
## a specified value.
static func world_state_property_equal_to(
		prop: String,
		value: Variant,
) -> Precondition:
	return _create_property_precondition(Target.WORLD_STATE, prop, Operation.EQUAL, value)


## Check if any of the agent's object data matches a requested group.
static func agent_has_object_data_of_group(group: String) -> Precondition:
	return Precondition.new(
		func(
			blackboard: GdPAIBlackboard,
			_world_state: GdPAIBlackboard
		):
		var objs: Array[GdPAIObjectData] = blackboard.get_objects_in_group(group)
		return objs.size() > 0
	)


## Check if any of the world state's object data matches a requested group.
static func world_state_has_object_data_of_group(group: String) -> Precondition:
	return Precondition.new(
		func(
			_blackboard: GdPAIBlackboard,
			world_state: GdPAIBlackboard
		):
		var objs: Array[GdPAIObjectData] = world_state.get_objects_in_group(group)
		return objs.size() > 0
	)


## Check if a given object is valid.
static func check_is_object_valid(object: Variant) -> Precondition:
	return Precondition.new(
		func(
			_blackboard: GdPAIBlackboard,
			_world_state: GdPAIBlackboard
		):
		return is_instance_valid(object)
	)


## Evaluates whether this precondition is satisfied by the given blackboard and world state.
func evaluate(
		blackboard: GdPAIBlackboard,
		world_state: GdPAIBlackboard,
) -> bool:
	if is_satisfied:
		return true
	is_satisfied = eval_func.call(blackboard, world_state)
	return is_satisfied


## Duplicate this object by creating a new instance and copying over all underlying data.
func copy_for_simulation() -> Precondition:
	var duplicate: Precondition = Precondition.new(eval_func)
	duplicate.is_satisfied = is_satisfied
	return duplicate
