class_name RequirementSpecBindingExists
extends RequirementSpec


var binding_name: String


func _init(name: String) -> void:
	binding_name = name


func to_bridge_dict() -> Dictionary:
	return {
		"kind": "binding_exists",
		"binding_name": binding_name,
	}
