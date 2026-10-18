class_name GdPAIWorldNode
extends Node
## Holds the shared world state. Add exactly one to the scene; agents find it when ready.

## Starting contents of the world blackboard. Required.
@export var blackboard_plan: GdPAIBlackboardPlan

## The live world blackboard, created from [member blackboard_plan] when the node is ready.
@onready var world_state: GdPAIBlackboard = blackboard_plan.generate_blackboard()


## Returns the world blackboard with its object list refreshed from the scene.
func get_world_state() -> GdPAIBlackboard:
	var objects: Array = []
	for obj in get_tree().get_nodes_in_group("GdPAIObjectData"):
		objects.append(obj)
	world_state.set_property("GDPAI_OBJECTS", objects)
	return world_state
