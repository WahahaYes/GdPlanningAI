## Denotes an interactable GdPAI object and maintains common associated attributes.
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
	assign_uid_and_entity(new_data)
	new_data.max_interaction_distance = max_interaction_distance
	return new_data
