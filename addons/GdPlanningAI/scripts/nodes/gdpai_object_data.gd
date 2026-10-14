class_name GdPAIObjectData
extends Node
## Base node that makes a scene object visible to the planner. Extend it for each kind of world
## object (a campfire, a food source) that agents should reason about or interact with.[br]
##[br]
## Each [GdPAIObjectData] joins the groups from [method get_group_labels] when created, and the
## [GdPAIWorldNode] (and each agent, for nodes below its entity) gathers all of them every plan
## request. For each plan request, on the main thread:[br]
## 1. [method get_provided_actions] is called to collect the [Action]s the object offers.[br]
## 2. [method get_sim_properties] is called once to snapshot the object into a [SimObjectProxy]
## that planning-time hooks read and write instead of the live node.[br]
## The snapshot's groups are the node's groups at that time, which is what group-based
## preconditions such as [method Precondition.world_object_property_geq_than] match.
## [codeblock]
## class_name CampfireObject
## extends GdPAIObjectData
##
## @export var fuel: float = 50.0
##
## func get_group_labels() -> Array[String]:
##     return ["CampfireObject", "GdPAIObjectData"]
##
## func get_sim_properties() -> Dictionary:
##     return {"current_fuel": fuel}
##
## func get_provided_actions() -> Array[Action]:
##     return [AddFuelAction.new(self)]
## [/codeblock]

## The top-level node of the object this data describes, for actions that need to reach the real
## object during execution.
@export var entity: Node


## Adds the node to every group returned by [method get_group_labels].
func _init() -> void:
	for label in get_group_labels():
		add_to_group(label)


## Returns the group names this node is added to when created. Groups are how preconditions and
## [method GdPAIBlackboard.get_proxy_in_group] find objects, so return one name per kind of object
## the planner should query.[br]
##[br]
## Called from [code]_init[/code], so it must not depend on exported values or the scene tree. An
## override must keep [code]"GdPAIObjectData"[/code] in the result (the world and agents find all
## objects through that group) and normally also the labels of its base class, as
## [GdPAIInteractable] does.
func get_group_labels() -> Array[String]:
	return ["GdPAIObjectData"]


## Returns the actions this object offers to agents, such as "pick up" or "add fuel".[br]
##[br]
## Called on the main thread each time an agent submits a plan request, so create fresh [Action]
## instances here (usually capturing [code]self[/code]) and keep it cheap. Return an empty array if
## the object offers nothing right now. Planning-time validity filtering is done by each
## action's [method Action.get_validity_checks].
func get_provided_actions() -> Array[Action]:
	return []


## Returns the properties of this object the planner may read or change, as a [Dictionary] of
## name to value.[br]
##[br]
## Called once per plan request on the main thread; the values are copied into a
## [SimObjectProxy], so later changes to the live node are not seen by that search. Use plain data
## (numbers, strings, vectors, node references for identity). Property names must match those used
## in preconditions and in [method Action.simulate_effect]. A string property named
## [code]provides[/code] additionally makes the object provide a fact of that name, with the
## object's instance id as its argument, to [method RequirementSpec.fact] requirements.
func get_sim_properties() -> Dictionary:
	return {}
