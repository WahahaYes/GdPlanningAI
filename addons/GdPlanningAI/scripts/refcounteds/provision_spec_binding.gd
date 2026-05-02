class_name ProvisionSpecBinding
extends ProvisionSpec
## Provides a binding with a specific value.


var binding_name: String
var value: Variant


func _init(name: String, val: Variant) -> void:
	binding_name = name
	value = val


func to_bridge_dict() -> Dictionary:
	return {
		"kind": "binding",
		"binding_name": binding_name,
		"value": value,
	}
