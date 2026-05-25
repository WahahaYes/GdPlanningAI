class_name PotatoObject
extends GdPAIObjectData

## Interactable metadata used by actions that target this potato.
@export var interactable_attribs: GdPAIInteractable
## Location metadata used by navigation requirements.
@export var location_data: GdPAILocationData


func get_group_labels() -> Array[String]:
	return ["PotatoObject", "Food", "GdPAIObjectData"]


func get_provided_actions() -> Array[Action]:
	return [DigPotatoAction.new(location_data, interactable_attribs, self)]


func get_sim_properties() -> Dictionary:
	return {}
