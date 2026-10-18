class_name RequirementSpecBindingInSet
extends RequirementSpec
## Requirement that a binding called [member binding_name] was provided with an object that
## belongs to the group [member set_name]. Create it with [method RequirementSpec.binding_in_set].

## Name of the required binding.
var binding_name: String
## Group the bound object must belong to.
var set_name: String


## Creates the requirement for binding [param name] in group [param target_set_name].
func _init(name: String, target_set_name: String) -> void:
	binding_name = name
	set_name = target_set_name


## Serialises this requirement for the Rust bridge.
func to_bridge_dict() -> Dictionary:
	return {
		"kind": "binding_in_set",
		"binding_name": binding_name,
		"set_name": set_name,
	}
