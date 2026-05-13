class_name CampfireObject
extends GdPAIObjectData

@export var interactable_attribs: GdPAIInteractable
@export var location_data: GdPAILocationData
@export var fuel_per_wood: float = 30.0
@export var min_fuel_to_cook: float = 20.0

var current_fuel: float = 100.0
var fuel_decay_rate: float = 3.0


func get_group_labels() -> Array[String]:
	return ["CampfireObject", "GdPAIObjectData"]


func get_provided_actions() -> Array[Action]:
	return [
		AddFuelAction.new(self , location_data, interactable_attribs, fuel_per_wood),
		CookPotatoAction.new(self , location_data, interactable_attribs, min_fuel_to_cook),
	]


func get_sim_properties() -> Dictionary:
	return {
		"fuel_per_wood": fuel_per_wood,
		"current_fuel": current_fuel,
		"min_fuel_to_cook": min_fuel_to_cook,
	}
