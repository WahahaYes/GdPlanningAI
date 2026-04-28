class_name RequirementSpecBindingInSet
extends RequirementSpec


var binding_name: String
var set_name: String


func _init(name: String, target_set_name: String) -> void:
	binding_name = name
	set_name = target_set_name


func to_bridge_dict() -> Dictionary:
	return {
		"kind": "binding_in_set",
		"binding_name": binding_name,
		"set_name": set_name,
	}
