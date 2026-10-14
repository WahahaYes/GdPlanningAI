class_name Precondition
extends RefCounted
## A condition on the agent blackboard, the world blackboard or world objects, evaluated against
## simulated state during planning. Used by [method Action.get_preconditions],
## [method Action.get_validity_checks] and [method Goal.get_desired_state].[br]
##[br]
## Create preconditions with the static constructors, never by instancing this class. Built-in
## constructors such as [method agent_property_greater_than] or
## [method world_object_property_geq_than] are evaluated natively by the planning engine and are
## the preferred choice: the planner can see which property they test and search for actions whose
## [method Action.simulate_effect] changes it. [method custom] and [method custom_with_deps] wrap
## your own [Callable]; the planner cannot look inside it, so it is run (on the main thread, once
## per candidate state it needs to check) as a black box, which is slower and gives the search no
## guidance. A custom precondition is called with the simulated agent and world blackboards and
## must return a [bool]; any other return value counts as [code]false[/code]. It must not use
## [code]await[/code] or the scene tree.
## [codeblock]
## var enough_wood: Precondition = Precondition.agent_property_geq_than("wood", 3)
## var near_home: Precondition = Precondition.custom(
##     func(agent_bb: GdPAIBlackboard, _world: GdPAIBlackboard) -> bool:
##         var me: SimObjectProxy = agent_bb.get_proxy_in_group("GdPAILocationData")
##         return me != null and me.get_property("position").length() < 10.0
## )
## [/codeblock]
## See also [PreconditionBuiltin], [PreconditionCustom] and [PreconditionCustomWithDeps].


## Returns the dictionary the Rust planning engine reads this precondition from. Called by the
## bridge on the main thread when a plan request is built. Implemented by
## [PreconditionBuiltin], [PreconditionCustom] and [PreconditionCustomWithDeps]; user code does not
## call or override it.
func to_bridge_dict() -> Dictionary:
	push_error("Precondition.to_bridge_dict must be overridden")
	return {}


## Creates a precondition that calls [param fn] with the simulated agent blackboard and world
## blackboard ([code]func(agent_bb: GdPAIBlackboard, world: GdPAIBlackboard) -> bool[/code]).
## The callable runs on the main thread during planning and must return a [bool]. Prefer a
## built-in constructor when one can express the condition.
static func custom(fn: Callable) -> Precondition:
	return PreconditionCustom.new(fn)


## Like [method custom], but also lists the live objects [param fn] captures in [param deps].
## If any of them has been freed, the precondition is [code]false[/code] and [param fn] is not
## called, which avoids errors from freed lambda captures.
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


## Check if any object in a group in the world state has a property greater than or equal to the given value.
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


## Check if any object in a group in the world state has a property less than or equal to the given value.
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


## True while [param object] has not been freed. Typically used in
## [method Action.get_validity_checks] to drop actions whose target is gone. The object is tracked
## as a dependency, so a freed object never reaches the check callable.
static func check_is_object_valid(object: Variant) -> Precondition:
	var deps: Array[Object] = []
	if object is Object:
		deps.append(object)

	var check: Callable = func(_b: GdPAIBlackboard, _w: GdPAIBlackboard) -> bool:
		return is_instance_valid(object)
	return PreconditionCustomWithDeps.new(check, deps)
