class_name GdPAIObjectData
extends Node
## Base node for denoting that an object is relevant for some agent AI and interaction.  Subclasses
## might hook into concrete actions (like grabbing or eating).

## Reference to the top-level node of this object, enabling referencing to the actual object during
## simulation.
@export var entity: Node

func _init() -> void:
	for label in get_group_labels():
		add_to_group(label)


## A list of group labels to be automatically assigned when this node is initialized.  This is done
## because the underlying grouping for Godot is a hashmap, so lookup is very fast.
func get_group_labels() -> Array[String]:
	return ["GdPAIObjectData"]


## If this node broadcasts any actions, instantiate these actions here.
func get_provided_actions() -> Array[Action]:
	return []


## Override this method to return a dictionary of properties that are relevant for simulation.
## This is captured once at the start of planning.
func get_sim_properties() -> Dictionary:
	return {}

