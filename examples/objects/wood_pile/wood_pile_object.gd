class_name WoodPileObject
extends GdPAIObjectData
## An endless wood supply for the campfire. Offers [PickUpWoodAction]; no simulated properties,
## since wood is never used up.

## Interactable metadata used by actions that target this wood pile.
@export var interactable_attribs: GdPAIInteractable
## Location metadata used by navigation requirements.
@export var location_data: GdPAILocationData


# Override
func get_group_labels() -> Array[String]:
	return ["WoodPileObject", "GdPAIObjectData"]


# Override
func get_provided_actions() -> Array[Action]:
	return [PickUpWoodAction.new(location_data, interactable_attribs)]


# Override
func get_sim_properties() -> Dictionary:
	return {}
