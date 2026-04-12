class_name FoodObject
extends GdPAIObjectData
## World object representing an edible food item.
##[br]
##[br]
## Demonstrates [GdPAIObjectData]: exposes sim properties and provides a
## [FoodAction] for any agent that can reach this item.


## How many points of hunger this item restores.
@export var hunger_value: float = 20.0
## How long it takes to eat this item in seconds.
@export var eating_duration: float = 1.0
## Reference to the [GdPAIInteractable] component on this object.
@export var interactable_attribs: GdPAIInteractable
## Reference to the [GdPAILocationData] component on this object.
@export var location_data: GdPAILocationData


# Override
func get_group_labels() -> Array[String]:
	return ["FoodObject", "GdPAIObjectData"]


# Override
func get_provided_actions() -> Array[Action]:
	return [FoodAction.new(location_data, interactable_attribs, self )]


# Override
func get_sim_properties() -> Dictionary:
	return {
		"hunger_value": hunger_value,
		"eating_duration": eating_duration,
	}
