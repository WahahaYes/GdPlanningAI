class_name RequirementSpecBindingExists
extends RequirementSpec
## Requires that a binding exists (regardless of value).


var binding_name: String


func _init(name: String) -> void:
	binding_name = name


func to_bridge_dict() -> Dictionary:
	return {
		"kind": "binding_exists",
		"binding_name": binding_name,
	}
