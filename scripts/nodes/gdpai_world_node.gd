## Provides the world state to GdPAI agents by querying the scene tree in order to compile up all
## GdPAI objects currently in the scene, and collect their provided actions.
class_name GdPAIWorldNode
extends Node

## A reference to utils for the GdPAI addon.
const GdPAIUTILS: Resource = preload("res://addons/GdPlanningAI/utils.gd")
## Blackboard plan for the world state.
@export var blackboard_plan: GdPAIBlackboardPlan
## The world state blackboard.
@onready var world_state: GdPAIBlackboard = blackboard_plan.generate_blackboard()


## When called, the scene tree is parsed to collect the active GdPAI object data.
func get_world_state() -> GdPAIBlackboard:
	var objects: Array[GdPAIObjectData] = []
	for obj: GdPAIObjectData in get_tree().get_nodes_in_group("GdPAIObjectData"):
		objects.append(obj)
	world_state.set_property(GdPAIBlackboard.GdPAI_OBJECTS, objects)
	return world_state
