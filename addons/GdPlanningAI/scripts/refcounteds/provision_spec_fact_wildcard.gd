class_name ProvisionSpecFactWildcard
extends ProvisionSpec
## Provides a fact that can satisfy any requirement with the same fact name,
## regardless of arguments. This enables a single action to satisfy multiple
## different specific requirements (e.g., a GoToAction that can navigate to any location).

var fact_name: String


func _init(name: String) -> void:
	fact_name = name


func to_bridge_dict() -> Dictionary:
	return {
		"kind": "fact_wildcard",
		"fact_name": fact_name,
	}
