class_name Precondition
extends RefCounted
## A condition on agent, world or object state, checked against simulated state during
## planning. Create one with the static constructors.[br]
## Prefer built-in constructors: the engine evaluates them natively and can search for actions
## that change the tested property. [method custom] callables are opaque to the search, run on the
## main thread, and must not use the scene tree or [code]await[/code].


## Returns the engine's view of this precondition. Implemented by subclasses; internal.
func to_bridge_dict() -> Dictionary:
	push_error("Precondition.to_bridge_dict must be overridden")
	return {}


## Creates a precondition from [param fn]:
## [code]func(agent_bb: GdPAIBlackboard, world: GdPAIBlackboard) -> bool[/code].
static func custom(fn: Callable) -> Precondition:
	return PreconditionCustom.new(fn)


## Like [method custom], but [code]false[/code] without calling [param fn] once any of
## [param deps] is freed.
static func custom_with_deps(fn: Callable, deps: Array[Object]) -> Precondition:
	return PreconditionCustomWithDeps.new(fn, deps)


## Shared factory behind the built-in constructors.
static func _create_property_precondition(
	target: PreconditionBuiltin.Target,
	prop: String,
	operation: PreconditionBuiltin.Op,
	value: Variant = null,
	grp: String = "",
) -> Precondition:
	return PreconditionBuiltin.new(target, operation, prop, value, grp)


## Instantiate a precondition that checks whether a property in the agent blackboard exists.
static func agent_has_property(prop: String) -> Precondition:
	return _create_property_precondition(
		PreconditionBuiltin.Target.AGENT, prop, PreconditionBuiltin.Op.HAS_PROPERTY
	)


## Instantiate a precondition that checks whether a property in the agent blackboard is not
## equal to a specified value.
static func agent_property_not_equal_to(
	prop: String,
	value: Variant,
) -> Precondition:
	return _create_property_precondition(
		PreconditionBuiltin.Target.AGENT, prop, PreconditionBuiltin.Op.NOT_EQUAL, value
	)


## Instantiate a precondition that checks whether a property in the agent blackboard is greater
## than a specified value.
static func agent_property_greater_than(
	prop: String,
	value: Variant,
) -> Precondition:
	return _create_property_precondition(
		PreconditionBuiltin.Target.AGENT, prop, PreconditionBuiltin.Op.GT, value
	)


## Instantiate a precondition that checks whether a property in the agent blackboard is greater
## or equal than a specified value.
static func agent_property_geq_than(
	prop: String,
	value: Variant,
) -> Precondition:
	return _create_property_precondition(
		PreconditionBuiltin.Target.AGENT, prop, PreconditionBuiltin.Op.GTE, value
	)


## Instantiate a precondition that checks whether a property in the agent blackboard is less
## than a specified value.
static func agent_property_less_than(
	prop: String,
	value: Variant,
) -> Precondition:
	return _create_property_precondition(
		PreconditionBuiltin.Target.AGENT, prop, PreconditionBuiltin.Op.LT, value
	)


## Instantiate a precondition that checks whether a property in the agent blackboard is less
## than or equal to a specified value.
static func agent_property_leq_than(
	prop: String,
	value: Variant,
) -> Precondition:
	return _create_property_precondition(
		PreconditionBuiltin.Target.AGENT, prop, PreconditionBuiltin.Op.LTE, value
	)


## Instantiate a precondition that checks whether a property in the agent blackboard is equal to
## a specified value.
static func agent_property_equal_to(
	prop: String,
	value: Variant,
) -> Precondition:
	return _create_property_precondition(
		PreconditionBuiltin.Target.AGENT, prop, PreconditionBuiltin.Op.EQUAL, value
	)


## Instantiate a precondition that checks whether a property in the world blackboard exists.
static func world_state_has_property(prop: String) -> Precondition:
	return _create_property_precondition(
		PreconditionBuiltin.Target.WORLD_STATE, prop, PreconditionBuiltin.Op.HAS_PROPERTY
	)


## Instantiate a precondition that checks whether a property in the world state is greater
## than a specified value.
static func world_state_property_greater_than(
	prop: String,
	value: Variant,
) -> Precondition:
	return _create_property_precondition(
		PreconditionBuiltin.Target.WORLD_STATE, prop, PreconditionBuiltin.Op.GT, value
	)


## Instantiate a precondition that checks whether a property in the world state is greater
## or equal than a specified value.
static func world_state_property_geq_than(
	prop: String,
	value: Variant,
) -> Precondition:
	return _create_property_precondition(
		PreconditionBuiltin.Target.WORLD_STATE, prop, PreconditionBuiltin.Op.GTE, value
	)


## Instantiate a precondition that checks whether a property in the world state is less
## than a specified value.
static func world_state_property_less_than(
	prop: String,
	value: Variant,
) -> Precondition:
	return _create_property_precondition(
		PreconditionBuiltin.Target.WORLD_STATE, prop, PreconditionBuiltin.Op.LT, value
	)


## Instantiate a precondition that checks whether a property in the world state is less
## than or equal to a specified value.
static func world_state_property_leq_than(
	prop: String,
	value: Variant,
) -> Precondition:
	return _create_property_precondition(
		PreconditionBuiltin.Target.WORLD_STATE, prop, PreconditionBuiltin.Op.LTE, value
	)


## Instantiate a precondition that checks whether a property in the world state is equal to
## a specified value.
static func world_state_property_equal_to(
	prop: String,
	value: Variant,
) -> Precondition:
	return _create_property_precondition(
		PreconditionBuiltin.Target.WORLD_STATE, prop, PreconditionBuiltin.Op.EQUAL, value
	)


## Check if any object in a group in the world state has a property equal to the given value.
static func world_object_property_equal_to(
	group: String,
	property: String,
	value: Variant,
) -> Precondition:
	return _create_property_precondition(
		PreconditionBuiltin.Target.WORLD_OBJECT_PROXY,
		property,
		PreconditionBuiltin.Op.EQUAL,
		value,
		group
	)


## Check if any object in a group in the world state has a property not equal to the given value.
static func world_object_property_not_equal_to(
	group: String,
	property: String,
	value: Variant,
) -> Precondition:
	return _create_property_precondition(
		PreconditionBuiltin.Target.WORLD_OBJECT_PROXY,
		property,
		PreconditionBuiltin.Op.NOT_EQUAL,
		value,
		group
	)


## Check if any object in a group in the world state has a property greater than the given value.
static func world_object_property_greater_than(
	group: String,
	property: String,
	value: Variant,
) -> Precondition:
	return _create_property_precondition(
		PreconditionBuiltin.Target.WORLD_OBJECT_PROXY,
		property,
		PreconditionBuiltin.Op.GT,
		value,
		group
	)


## Check if any object in a group in the world state has a property greater than or equal to
## the given value.
static func world_object_property_geq_than(
	group: String,
	property: String,
	value: Variant,
) -> Precondition:
	return _create_property_precondition(
		PreconditionBuiltin.Target.WORLD_OBJECT_PROXY,
		property,
		PreconditionBuiltin.Op.GTE,
		value,
		group
	)


## Check if any object in a group in the world state has a property less than the given value.
static func world_object_property_less_than(
	group: String,
	property: String,
	value: Variant,
) -> Precondition:
	return _create_property_precondition(
		PreconditionBuiltin.Target.WORLD_OBJECT_PROXY,
		property,
		PreconditionBuiltin.Op.LT,
		value,
		group
	)


## Check if any object in a group in the world state has a property less than or equal to the
## given value.
static func world_object_property_leq_than(
	group: String,
	property: String,
	value: Variant,
) -> Precondition:
	return _create_property_precondition(
		PreconditionBuiltin.Target.WORLD_OBJECT_PROXY,
		property,
		PreconditionBuiltin.Op.LTE,
		value,
		group
	)


## Check if any object in a group in the world state has the specified property.
static func world_object_has_property(group: String, property: String) -> Precondition:
	return _create_property_precondition(
		PreconditionBuiltin.Target.WORLD_OBJECT_PROXY,
		property,
		PreconditionBuiltin.Op.HAS_PROPERTY,
		null,
		group
	)


## True if the agent owns at least one object data node (such as a [GdPAILocationData]) in
## [param group], i.e. a simulated proxy for it exists on the agent blackboard.
static func agent_has_object_data_of_group(group: String) -> Precondition:
	var check: Callable = func(bb: GdPAIBlackboard, _w: GdPAIBlackboard):
		return bb.get_proxies_in_group(group).size() > 0
	return PreconditionCustom.new(check)


## True if the world state contains at least one object data node in [param group].
static func world_state_has_object_data_of_group(group: String) -> Precondition:
	var check: Callable = func(_b: GdPAIBlackboard, ws: GdPAIBlackboard):
		return ws.get_proxies_in_group(group).size() > 0
	return PreconditionCustom.new(check)


## True while [param object] has not been freed. Typical in [method Action.get_validity_checks].
static func check_is_object_valid(object: Variant) -> Precondition:
	var deps: Array[Object] = []
	if is_instance_valid(object) and object is Object:
		deps.append(object)

	var check: Callable = func(_b: GdPAIBlackboard, _w: GdPAIBlackboard) -> bool:
		return is_instance_valid(object)
	return PreconditionCustomWithDeps.new(check, deps)
