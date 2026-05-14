class_name WoodPileObject
extends GdPAIObjectData

## Interactable metadata used by actions that target this wood pile.
@export var interactable_attribs: GdPAIInteractable
## Location metadata used by navigation requirements.
@export var location_data: GdPAILocationData


func get_group_labels() -> Array[String]:
	return ["WoodPileObject", "GdPAIObjectData"]


func get_provided_actions() -> Array[Action]:
	return [PickUpWoodAction.new(location_data, interactable_attribs)]


func get_sim_properties() -> Dictionary:
	return {}
