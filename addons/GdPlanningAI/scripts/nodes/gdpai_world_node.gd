class_name GdPAIWorldNode
extends Node
## Holds the shared world state. Add exactly one to the scene; agents find it when ready.

## Starting contents of the world blackboard. Required.
@export var blackboard_plan: GdPAIBlackboardPlan

## Process frame the cached object list was last rebuilt in. [method get_world_state]
## rebuilds at most once per frame no matter how many agents plan in it.
var _last_refresh_frame: int = -1

## The live world blackboard, created from [member blackboard_plan] when the node is ready.
@onready var world_state: GdPAIBlackboard = blackboard_plan.generate_blackboard()


## Returns the world blackboard, rebuilding its cached object list first when no
## refresh ran yet this frame. Rebuilding re-snapshots every object's simulation
## properties, which change without any tree churn (burning fuel, moving bodies),
## so the list cannot refresh on enter/exit alone; every agent planning in the same
## frame shares the one refresh.
func get_world_state() -> GdPAIBlackboard:
	if world_state == null:
		return null
	if Engine.get_process_frames() != _last_refresh_frame:
		_refresh_objects()
	return world_state


## Rebuilds the cached object list from the scene tree. The array must stay untyped:
## the Rust bridge only converts untyped arrays into object snapshots, so passing the
## typed array from [method SceneTree.get_nodes_in_group] straight in would silently
## leave every plan with an empty world.
func _refresh_objects() -> void:
	_last_refresh_frame = Engine.get_process_frames()
	var tree: SceneTree = get_tree()
	if tree == null:
		return
	var objects: Array = []
	objects.append_array(tree.get_nodes_in_group("GdPAIObjectData"))
	world_state.set_property("GDPAI_OBJECTS", objects)
