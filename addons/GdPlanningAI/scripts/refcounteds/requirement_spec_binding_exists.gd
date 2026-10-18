class_name RequirementSpecBindingExists
extends RequirementSpec
## Requirement that a binding called [member binding_name] was provided, whatever its value.
## Create it with [method RequirementSpec.binding_exists].

## Name of the required binding.
var binding_name: String


## Creates the requirement for binding [param name].
func _init(name: String) -> void:
	binding_name = name


## Serialises this requirement for the Rust bridge.
func to_bridge_dict() -> Dictionary:
	return {
		"kind": "binding_exists",
		"binding_name": binding_name,
	}
