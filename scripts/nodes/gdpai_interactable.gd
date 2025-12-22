class_name GdPAIInteractable
extends GdPAIObjectData
## Denotes an interactable GdPAI object and maintains common associated attributes.

## An object can specify how far away it need be.  Values <=0 disable the check.
@export var max_interaction_distance: float = 2
## The maximum distance an object can move from its planning-time position before
## an associated spatial action is failed.
## Values less than 0 (n<0) never trigger failure.
@export var max_drift_from_plan: float = -1

## The maximum distance an object can move from its planning-time position before
## an associated spatial action is failed.
## Values less than 0 (n<0) never trigger failure.
@export var max_drift_from_plan: float = -1


# Override
func get_group_labels() -> Array[String]:
	return ["GdPAIInteractable", "GdPAIObjectData"]


# Override
func copy_for_simulation() -> GdPAIObjectData:
	var new_data: GdPAIInteractable = GdPAIInteractable.new()
	assign_uid_and_entity(new_data)
	new_data.max_interaction_distance = max_interaction_distance
	new_data.max_drift_from_plan = max_drift_from_plan
	return new_data
