class_name CampfireObject
extends GdPAIObjectData

## Interactable metadata used by actions that target this campfire.
@export var interactable_attribs: GdPAIInteractable
## Location metadata used by navigation requirements.
@export var location_data: GdPAILocationData
## Fuel added to the campfire when one wood item is consumed.
@export var fuel_per_wood: float = 30.0
## Minimum fuel required before food can be cooked.
@export var min_fuel_to_cook: float = 20.0

var current_fuel: float = 100.0
var fuel_decay_rate: float = 3.0


func get_group_labels() -> Array[String]:
	return ["CampfireObject", "GdPAIObjectData"]


func get_provided_actions() -> Array[Action]:
	return [
		AddFuelAction.new(self, location_data, interactable_attribs, fuel_per_wood),
		CookPotatoAction.new(self, location_data, interactable_attribs, min_fuel_to_cook),
	]


func get_sim_properties() -> Dictionary:
	return {
		"fuel_per_wood": fuel_per_wood,
		"current_fuel": current_fuel,
		"min_fuel_to_cook": min_fuel_to_cook,
	}
