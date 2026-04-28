class_name RequirementSpecBindingEquals
extends RequirementSpec


var binding_name: String
var value: Variant


func _init(name: String, val: Variant) -> void:
	binding_name = name
	value = val


func to_bridge_dict() -> Dictionary:
	return {
		"kind": "binding_equals",
		"binding_name": binding_name,
		"value": value,
	}
