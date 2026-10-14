class_name GdPAIWorldNode
extends Node
## Provides the shared world state to all agents. Add exactly one to the scene and assign its
## [member blackboard_plan]; agents locate it by searching the scene tree when they become ready,
## and do not plan without one.[br]
##[br]
## Each time an agent plans, [method get_world_state] refreshes the world blackboard's list of
## objects with every node in the group [code]GdPAIObjectData[/code] currently in the tree. The
## agent then collects those objects' provided actions and snapshots their simulation properties.

## Initial contents of the world blackboard. Required: the node cannot create its world state
## without it.
@export var blackboard_plan: GdPAIBlackboardPlan

## The live world blackboard, created from [member blackboard_plan] when the node is ready.
@onready var world_state: GdPAIBlackboard = blackboard_plan.generate_blackboard()


## Returns the world blackboard after refreshing its [code]GDPAI_OBJECTS[/code] list from the
## group [code]GdPAIObjectData[/code]. Called on the main thread each time an agent submits a
## plan request.
func get_world_state() -> GdPAIBlackboard:
	var objects: Array = []
	for obj in get_tree().get_nodes_in_group("GdPAIObjectData"):
		objects.append(obj)
	world_state.set_property("GDPAI_OBJECTS", objects)
	return world_state
