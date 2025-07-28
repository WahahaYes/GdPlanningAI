## GdPAI food item example.  Stores the amount of hunger restored and broadcasts that this item can
## be eaten.
class_name SampleFoodObject
extends GdPAIObjectData

## How many points of hunger this restores.
@export var hunger_value: float = 5
## How long it takes to eat this item.
@export var eating_duration: float = 1

## Reference to GdPAI interactable.
@export var interactable_attribs: GdPAIInteractable
## Reference to GdPAI location data.
@export var location_data: GdPAILocationData


# Override
func get_group_labels():
	# Make sure to add a group label for this class of data.
	return ["SampleFoodObject", "GdPAIObjectData"]


# Override
func get_provided_actions() -> Array[Action]:
	# Overwrite the get_provided_actions function to serve any actions that become possible because
	# this object exists out in the world.
	var food_action: SampleFoodAction = SampleFoodAction.new(
		location_data, interactable_attribs, self
	)
	return [food_action]


# Override
func copy_for_simulation() -> GdPAIObjectData:
	# Make sure to replace <GdPAIObjectData> with the subclass name, and to duplicate any new properties.
	var new_data: SampleFoodObject = SampleFoodObject.new()
	assign_uid_and_entity(new_data)
	new_data.hunger_value = hunger_value
	new_data.eating_duration = eating_duration
	return new_data
