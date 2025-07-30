## Base node for denoting that an object is relevant for some agent AI and interaction.  Subclasses
## might hook into concrete actions (like grabbing or eating).
class_name GdPAIObjectData
extends Node

## A reference to utils for the GdPAI addon.
const GdPAIUTILS: Resource = preload("res://addons/GdPlanningAI/utils.gd")
## A uid is automatically allocated so that the actual object can be tracked throughout the
## simulated blackboards as they are duplicated.  These are copied over in copy_for_simulation().
var uid: String
## Counter for allocating new uids.
static var _uid_counter: int = 0
## Reference to the top-level node of this object, enabling referencing to the actual object during
## simulation.
@export var entity: Node


func _init() -> void:
	# The uid and groups are assigned when this node is initialized.
	uid = str(_uid_counter)
	_uid_counter += 1
	for label in get_group_labels():
		add_to_group(label)


## A list of group labels to be automatically assigned when this node is initialized.  This is done
## because the underlying grouping for Godot is a hashmap, so lookup is very fast.
func get_group_labels() -> Array[String]:
	return ["GdPAIObjectData"]


## If this node broadcasts any actions, instantiate these actions here.
func get_provided_actions() -> Array[Action]:
	return []


## Duplicate the data about the object without altering the physical thing.  Make sure to clone
## anything that would be relevant for agent planning or altered during simulation.
func copy_for_simulation() -> GdPAIObjectData:
	var new_data: GdPAIObjectData = GdPAIObjectData.new()
	assign_uid_and_entity(new_data)
	return new_data


## Reassigns the original's uid and copies over the entity if valid.  Call this when extending
## copy_for_simulation().
func assign_uid_and_entity(new_data: GdPAIObjectData):
	new_data.uid = uid  # Restore the original's uid so the clone can be tracked in simulation.
	if entity == null or not is_instance_valid(entity):
		new_data.entity = null
	else:
		new_data.entity = entity
