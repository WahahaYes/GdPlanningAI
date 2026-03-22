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
## var precondition = Precondition.custom(func(blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):[br]
##     if blackboard.get_property(<prop>) and world_state.get_property(<prop>):[br]
##         return true[br]
##     return false[br]
## )[br]
##[br]
##[br]
## (The above example would check that a property exists / is true for both the agent and world
## states.)

## Evaluates whether this precondition is satisfied by the given blackboard and world state.
func evaluate(
		agent: GdPAIBlackboard,
		world: GdPAIBlackboard,
) -> bool:
	return _do_evaluate(agent, world)


func _do_evaluate(_agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> bool:
	push_error("Precondition._do_evaluate must be overridden")
	return false


static func custom(fn: Callable) -> PreconditionCustom:
	return PreconditionCustom.new(fn)


## Generic function to create property comparison preconditions, eliminating code duplication.
static func _create_property_precondition(
		target: PreconditionBuiltin.Target,
		prop: String,
		operation: PreconditionBuiltin.Op,
		value: Variant = null,
) -> PreconditionBuiltin:
	return PreconditionBuiltin.new(target, operation, prop, value)


## Instantiate a precondition that checks whether a property in the agent blackboard exists.
static func agent_has_property(prop: String) -> PreconditionBuiltin:
	return _create_property_precondition(PreconditionBuiltin.Target.AGENT, prop, PreconditionBuiltin.Op.HAS_PROPERTY)


## Instantiate a precondition that checks whether a property in the agent blackboard is not
## equal to a specified value.
static func agent_property_not_equal_to(
		prop: String,
		value: Variant,
) -> PreconditionBuiltin:
	return _create_property_precondition(PreconditionBuiltin.Target.AGENT, prop, PreconditionBuiltin.Op.NOT_EQUAL, value)


## Instantiate a precondition that checks whether a property in the agent blackboard is greater
## than a specified value.
static func agent_property_greater_than(
		prop: String,
		value: Variant,
) -> PreconditionBuiltin:
	return _create_property_precondition(PreconditionBuiltin.Target.AGENT, prop, PreconditionBuiltin.Op.GT, value)


## Instantiate a precondition that checks whether a property in the agent blackboard is greater
## or equal than a specified value.
static func agent_property_geq_than(
		prop: String,
		value: Variant,
) -> PreconditionBuiltin:
	return _create_property_precondition(PreconditionBuiltin.Target.AGENT, prop, PreconditionBuiltin.Op.GTE, value)


## Instantiate a precondition that checks whether a property in the agent blackboard is less
## than a specified value.
static func agent_property_less_than(
		prop: String,
		value: Variant,
) -> PreconditionBuiltin:
	return _create_property_precondition(PreconditionBuiltin.Target.AGENT, prop, PreconditionBuiltin.Op.LT, value)


## Instantiate a precondition that checks whether a property in the agent blackboard is less
## than or equal to a specified value.
static func agent_property_leq_than(
		prop: String,
		value: Variant,
) -> PreconditionBuiltin:
	return _create_property_precondition(PreconditionBuiltin.Target.AGENT, prop, PreconditionBuiltin.Op.LTE, value)


## Instantiate a precondition that checks whether a property in the agent blackboard is equal to
## a specified value.
static func agent_property_equal_to(
		prop: String,
		value: Variant,
) -> PreconditionBuiltin:
	return _create_property_precondition(PreconditionBuiltin.Target.AGENT, prop, PreconditionBuiltin.Op.EQUAL, value)


## Instantiate a precondition that checks whether a property in the agent blackboard exists.
static func world_state_has_property(prop: String) -> PreconditionBuiltin:
	return _create_property_precondition(PreconditionBuiltin.Target.WORLD_STATE, prop, PreconditionBuiltin.Op.HAS_PROPERTY)


## Instantiate a precondition that checks whether a property in the world state is greater
## than a specified value.
static func world_state_property_greater_than(
		prop: String,
		value: Variant,
) -> PreconditionBuiltin:
	return _create_property_precondition(PreconditionBuiltin.Target.WORLD_STATE, prop, PreconditionBuiltin.Op.GT, value)


## Instantiate a precondition that checks whether a property in the world state is greater
## or equal than a specified value.
static func world_state_property_geq_than(
		prop: String,
		value: Variant,
) -> PreconditionBuiltin:
	return _create_property_precondition(
		PreconditionBuiltin.Target.WORLD_STATE,
		prop,
		PreconditionBuiltin.Op.GTE,
		value,
	)


## Instantiate a precondition that checks whether a property in the world state is less
## than a specified value.
static func world_state_property_less_than(
		prop: String,
		value: Variant,
) -> PreconditionBuiltin:
	return _create_property_precondition(PreconditionBuiltin.Target.WORLD_STATE, prop, PreconditionBuiltin.Op.LT, value)


## Instantiate a precondition that checks whether a property in the world state is less
## than or equal to a specified value.
static func world_state_property_leq_than(
		prop: String,
		value: Variant,
) -> PreconditionBuiltin:
	return _create_property_precondition(
		PreconditionBuiltin.Target.WORLD_STATE,
		prop,
		PreconditionBuiltin.Op.LTE,
		value,
	)


## Instantiate a precondition that checks whether a property in the world state is equal to
## a specified value.
static func world_state_property_equal_to(
		prop: String,
		value: Variant,
) -> PreconditionBuiltin:
	return _create_property_precondition(PreconditionBuiltin.Target.WORLD_STATE, prop, PreconditionBuiltin.Op.EQUAL, value)


## Check if any of the agent's object data matches a requested group.
static func agent_has_object_data_of_group(group: String) -> PreconditionCustom:
	return PreconditionCustom.new(
		func(
			blackboard: GdPAIBlackboard,
			_world_state: GdPAIBlackboard
		):
		var objs: Array[SimObjectProxy] = blackboard.get_objects_in_group(group)
		return objs.size() > 0
	)


## Check if any of the world state's object data matches a requested group.
static func world_state_has_object_data_of_group(group: String) -> PreconditionCustom:
	return PreconditionCustom.new(
		func(
			_blackboard: GdPAIBlackboard,
			world_state: GdPAIBlackboard
		):
		var objs: Array[SimObjectProxy] = world_state.get_objects_in_group(group)
		return objs.size() > 0
	)


## Check if a given object is valid.
static func check_is_object_valid(object: Variant) -> PreconditionCustom:
	return PreconditionCustom.new(
		func(
			_blackboard: GdPAIBlackboard,
			_world_state: GdPAIBlackboard
		):
		return is_instance_valid(object)
	)
