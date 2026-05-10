class_name PotatoObject
extends GdPAIObjectData

@export var interactable_attribs: GdPAIInteractable
@export var location_data: GdPAILocationData


func get_group_labels() -> Array[String]:
	return ["PotatoObject", "GdPAIObjectData"]


func get_provided_actions() -> Array[Action]:
	return [DigPotatoAction.new(location_data, interactable_attribs, self)]


func get_sim_properties() -> Dictionary:
	return {}
