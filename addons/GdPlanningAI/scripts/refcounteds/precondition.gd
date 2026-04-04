class_name Precondition
extends RefCounted
## Preconditions for planning chains of actions.  This class allows for custom preconditions
## to be defined and evaluated at planning-time.
##[br]
##[br]
## There are static methods, like Precondition.agent_property_greater_than(<prop>, <value>),
## which will instantiate common preconditions in a single line of code.  Alterantively, custom
## preconditions can be defined like so:
##[br]
##[br]
## var precondition = Precondition.custom(
##         func(blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):[br]
##     if blackboard.get_property(<prop>) and world_state.get_property(<prop>):[br]
##         return true[br]
##     return false[br]
## )[br]
##[br]
##[br]
## (The above example would check that a property exists / is true for both the agent and world
## states.)

## Returns the dictionary representation of this precondition for the Rust bridge.
## Implemented by [PreconditionBuiltin] and [PreconditionCustom].
func to_bridge_dict() -> Dictionary:
	push_error("Precondition.to_bridge_dict must be overridden")
	return {}

## Create a custom precondition that invokes a callable with the agent and world blackboards.
static func custom(fn: Callable) -> Precondition:
	return PreconditionCustom.new(fn)


## Create a custom precondition with explicit object dependencies.
## The planner will validate these objects exist before invoking the precondition.
static func custom_with_deps(fn: Callable, deps: Array[Object]) -> Precondition:
	return PreconditionCustomWithDeps.new(fn, deps)


## Generic function to create property comparison preconditions, eliminating code duplication.
static func _create_property_precondition(
		target: PreconditionBuiltin.Target,
		prop: String,
		operation: PreconditionBuiltin.Op,
		value: Variant = null,
) -> Precondition:
	return PreconditionBuiltin.new(target, operation, prop, value)


## Instantiate a precondition that checks whether a property in the agent blackboard exists.
static func agent_has_property(prop: String) -> Precondition:
	return _create_property_precondition(
		PreconditionBuiltin.Target.AGENT, prop, PreconditionBuiltin.Op.HAS_PROPERTY)


## Instantiate a precondition that checks whether a property in the agent blackboard is not
## equal to a specified value.
static func agent_property_not_equal_to(
		prop: String,
		value: Variant,
) -> Precondition:
	return _create_property_precondition(
		PreconditionBuiltin.Target.AGENT, prop, PreconditionBuiltin.Op.NOT_EQUAL, value)


## Instantiate a precondition that checks whether a property in the agent blackboard is greater
## than a specified value.
static func agent_property_greater_than(
		prop: String,
		value: Variant,
) -> Precondition:
	return _create_property_precondition(
		PreconditionBuiltin.Target.AGENT, prop, PreconditionBuiltin.Op.GT, value)


## Instantiate a precondition that checks whether a property in the agent blackboard is greater
## or equal than a specified value.
static func agent_property_geq_than(
		prop: String,
		value: Variant,
) -> Precondition:
	return _create_property_precondition(
		PreconditionBuiltin.Target.AGENT, prop, PreconditionBuiltin.Op.GTE, value)


## Instantiate a precondition that checks whether a property in the agent blackboard is less
## than a specified value.
static func agent_property_less_than(
		prop: String,
		value: Variant,
) -> Precondition:
	return _create_property_precondition(
		PreconditionBuiltin.Target.AGENT, prop, PreconditionBuiltin.Op.LT, value)


## Instantiate a precondition that checks whether a property in the agent blackboard is less
## than or equal to a specified value.
static func agent_property_leq_than(
		prop: String,
		value: Variant,
) -> Precondition:
	return _create_property_precondition(
		PreconditionBuiltin.Target.AGENT, prop, PreconditionBuiltin.Op.LTE, value)


## Instantiate a precondition that checks whether a property in the agent blackboard is equal to
## a specified value.
static func agent_property_equal_to(
		prop: String,
		value: Variant,
) -> Precondition:
	return _create_property_precondition(
		PreconditionBuiltin.Target.AGENT, prop, PreconditionBuiltin.Op.EQUAL, value)


## Instantiate a precondition that checks whether a property in the agent blackboard exists.
static func world_state_has_property(prop: String) -> Precondition:
	return _create_property_precondition(
		PreconditionBuiltin.Target.WORLD_STATE, prop, PreconditionBuiltin.Op.HAS_PROPERTY)


## Instantiate a precondition that checks whether a property in the world state is greater
## than a specified value.
static func world_state_property_greater_than(
		prop: String,
		value: Variant,
) -> Precondition:
	return _create_property_precondition(
		PreconditionBuiltin.Target.WORLD_STATE, prop, PreconditionBuiltin.Op.GT, value)


## Instantiate a precondition that checks whether a property in the world state is greater
## or equal than a specified value.
static func world_state_property_geq_than(
		prop: String,
		value: Variant,
) -> Precondition:
	return _create_property_precondition(
		PreconditionBuiltin.Target.WORLD_STATE, prop, PreconditionBuiltin.Op.GTE, value)


## Instantiate a precondition that checks whether a property in the world state is less
## than a specified value.
static func world_state_property_less_than(
		prop: String,
		value: Variant,
) -> Precondition:
	return _create_property_precondition(
		PreconditionBuiltin.Target.WORLD_STATE, prop, PreconditionBuiltin.Op.LT, value)


## Instantiate a precondition that checks whether a property in the world state is less
## than or equal to a specified value.
static func world_state_property_leq_than(
		prop: String,
		value: Variant,
) -> Precondition:
	return _create_property_precondition(
		PreconditionBuiltin.Target.WORLD_STATE, prop, PreconditionBuiltin.Op.LTE, value)


## Instantiate a precondition that checks whether a property in the world state is equal to
## a specified value.
static func world_state_property_equal_to(
		prop: String,
		value: Variant,
) -> Precondition:
	return _create_property_precondition(
		PreconditionBuiltin.Target.WORLD_STATE, prop, PreconditionBuiltin.Op.EQUAL, value)


## Check if any of the agent's object data matches a requested group.
static func agent_has_object_data_of_group(group: String) -> Precondition:
	return PreconditionCustom.new(
		func(
			blackboard: GdPAIBlackboard,
			_world_state: GdPAIBlackboard
		):
		var objs: Array[SimObjectProxy] = blackboard.get_proxies_in_group(group)
		return objs.size() > 0
	)


## Check if any of the world state's object data matches a requested group.
static func world_state_has_object_data_of_group(group: String) -> Precondition:
	return PreconditionCustom.new(
		func(
			_blackboard: GdPAIBlackboard,
			world_state: GdPAIBlackboard
		):
		var objs: Array[SimObjectProxy] = world_state.get_proxies_in_group(group)
		return objs.size() > 0
	)


## Check if a given object is valid.
## Uses dependency tracking to validate the object exists before planning.
static func check_is_object_valid(object: Variant) -> Precondition:
	var deps: Array[Object] = []
	if object is Object:
		deps.append(object)
	
	return PreconditionCustomWithDeps.new(
		func(
			_blackboard: GdPAIBlackboard,
			_world_state: GdPAIBlackboard
		) -> bool:
			return is_instance_valid(object),
		deps
	)
