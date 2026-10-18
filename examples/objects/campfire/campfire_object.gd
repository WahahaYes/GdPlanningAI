class_name CampfireObject
extends GdPAIObjectData
## A campfire: fuel agents keep topped up and cook at.[br]
## It offers its own actions ([AddFuelAction], [CookPotatoAction]), so agent configs never
## mention it: drop a campfire into a scene and every agent can use it. Burn-down and visuals
## live in [CampfireRuntime].

## Interactable metadata used by actions that target this campfire.
@export var interactable_attribs: GdPAIInteractable
## Location used as this fire's [code]at_target[/code].
@export var location_data: GdPAILocationData
## Fuel added to the campfire when one wood item is consumed.
@export var fuel_per_wood: float = 30.0
## Minimum fuel required before food can be cooked.
@export var min_fuel_to_cook: float = 20.0

## Starting fuel in percent. Half-full so fire maintenance starts quickly.
@export var current_fuel: float = 50.0
## Fuel lost per second, applied by [CampfireRuntime].
var fuel_decay_rate: float = 3.0


# Override
func get_group_labels() -> Array[String]:
	return ["CampfireObject", "GdPAIObjectData"]


# Override
func get_provided_actions() -> Array[Action]:
	return [
		AddFuelAction.new(self, location_data, interactable_attribs, fuel_per_wood),
		CookPotatoAction.new(self, location_data, interactable_attribs, min_fuel_to_cook),
	]


# Override
func get_sim_properties() -> Dictionary:
	return {
		"fuel_per_wood": fuel_per_wood,
		"current_fuel": current_fuel,
		"min_fuel_to_cook": min_fuel_to_cook,
	}
