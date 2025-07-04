## Denotes an interactable APP object and associated attributes needed for navigation.
class_name GdPAIInteractable
extends GdPAIObjectData

## An object can specify how far away it need be.  Values <=0 disable the check.
@export var max_interaction_distance: float = 2


# Override
func get_group_labels():
	return ["GdPAIInteractable", "GdPAIObjectData"]


# Override
func copy_for_simulation() -> GdPAIObjectData:
	var new_data: GdPAIInteractable = GdPAIInteractable.new()
	new_data.uid = uid
	new_data.entity = entity
	return new_data
