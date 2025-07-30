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
class_name Precondition
extends RefCounted

## A reference to utils for the GdPAI addon.
const GdPAIUTILS: Resource = preload("res://addons/GdPlanningAI/utils.gd")
## Variable to keep track of whether earlier evaluations of this precondition were successful.
var is_satisfied: bool = false
## The evaluation function is dynamic and can be set by instantiating the precondition using one
## of the static methods, or by defining a custom check.
var eval_func: Callable


## Evaluates whether this precondition is satisfied by the given blackboard and world state.
func evaluate(blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard) -> bool:
	if is_satisfied:
		return true
	is_satisfied = eval_func.call(blackboard, world_state)
	return is_satisfied


## Duplicate this object by creating a new instance and copying over all underlying data.
func copy_for_simulation():
	var duplicate = Precondition.new()
	duplicate.is_satisfied = is_satisfied
	duplicate.eval_func = eval_func
	return duplicate


#region
# Agent property value comparsions.


## Instantiate a precondition that checks whether a property in the agent blackboard exists.
static func agent_has_property(prop: String) -> Precondition:
	var precondition: Precondition = Precondition.new()
	precondition.eval_func = func(blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):
		return prop in blackboard.get_dict()
	return precondition


## Instantiate a precondition that checks whether a property in the agent blackboard is greater
## than a specified value.
static func agent_property_greater_than(prop: String, value: Variant) -> Precondition:
	var precondition: Precondition = Precondition.new()
	precondition.eval_func = func(blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):
		return blackboard.get_property(prop) > value
	return precondition


## Instantiate a precondition that checks whether a property in the agent blackboard is greater
## or equal than a specified value.
static func agent_property_geq_than(prop: String, value: Variant) -> Precondition:
	var precondition: Precondition = Precondition.new()
	precondition.eval_func = func(blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):
		return blackboard.get_property(prop) > value
	return precondition


## Instantiate a precondition that checks whether a property in the agent blackboard is less
## than a specified value.
static func agent_property_less_than(prop: String, value: Variant) -> Precondition:
	var precondition: Precondition = Precondition.new()
	precondition.eval_func = func(blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):
		return blackboard.get_property(prop) < value
	return precondition


## Instantiate a precondition that checks whether a property in the agent blackboard is less
## than or equal to a specified value.
static func agent_property_leq_than(prop: String, value: Variant) -> Precondition:
	var precondition: Precondition = Precondition.new()
	precondition.eval_func = func(blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):
		return blackboard.get_property(prop) <= value
	return precondition


## Instantiate a precondition that checks whether a property in the agent blackboard is equal to
## a specified value.
static func agent_property_equal_to(prop: String, value: Variant) -> Precondition:
	var precondition: Precondition = Precondition.new()
	precondition.eval_func = func(blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):
		return blackboard.get_property(prop) == value
	return precondition


#endregion
#region
# World state property value comparsions.


## Instantiate a precondition that checks whether a property in the agent blackboard exists.
static func world_state_has_property(prop: String) -> Precondition:
	var precondition: Precondition = Precondition.new()
	precondition.eval_func = func(blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):
		return prop in world_state.get_dict()
	return precondition


## Instantiate a precondition that checks whether a property in the agent blackboard is greater
## than a specified value.
static func world_state_property_greater_than(prop: String, value: Variant) -> Precondition:
	var precondition: Precondition = Precondition.new()
	precondition.eval_func = func(blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):
		return world_state.get_property(prop) > value
	return precondition


## Instantiate a precondition that checks whether a property in the agent blackboard is greater
## or equal than a specified value.
static func world_state_property_geq_than(prop: String, value: Variant) -> Precondition:
	var precondition: Precondition = Precondition.new()
	precondition.eval_func = func(blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):
		return world_state.get_property(prop) > value
	return precondition


## Instantiate a precondition that checks whether a property in the agent blackboard is less
## than a specified value.
static func world_state_property_less_than(prop: String, value: Variant) -> Precondition:
	var precondition: Precondition = Precondition.new()
	precondition.eval_func = func(blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):
		return world_state.get_property(prop) < value
	return precondition


## Instantiate a precondition that checks whether a property in the agent blackboard is less
## than or equal to a specified value.
static func world_state_property_leq_than(prop: String, value: Variant) -> Precondition:
	var precondition: Precondition = Precondition.new()
	precondition.eval_func = func(blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):
		return world_state.get_property(prop) <= value
	return precondition


## Instantiate a precondition that checks whether a property in the agent blackboard is equal to
## a specified value.
static func world_state_property_equal_to(prop: String, value: Variant) -> Precondition:
	var precondition: Precondition = Precondition.new()
	precondition.eval_func = func(blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):
		return world_state.get_property(prop) == value
	return precondition


#endregion
#region
# More complicated common checks.


## Check if any of the agent's object data matches a requested group.
static func agent_has_object_data_of_group(group: String) -> Precondition:
	var precondition: Precondition = Precondition.new()
	precondition.eval_func = func(blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):
		var objs: Array[GdPAIObjectData] = blackboard.get_objects_in_group(group)
		return objs.size() > 0

	return precondition


## Check if any of the world state's object data matches a requested group.
static func world_state_has_object_data_of_group(group: String) -> Precondition:
	var precondition: Precondition = Precondition.new()
	precondition.eval_func = func(blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):
		var objs: Array[GdPAIObjectData] = world_state.get_objects_in_group(group)
		return objs.size() > 0

	return precondition


## Check if a given object is valid
static func check_is_object_valid(object: Variant) -> Precondition:
	var precondition: Precondition = Precondition.new()
	precondition.eval_func = func(blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):
		return is_instance_valid(object)

	return precondition

#endregion
