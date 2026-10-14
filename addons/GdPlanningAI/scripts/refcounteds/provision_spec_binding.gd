class_name ProvisionSpecBinding
extends ProvisionSpec
## Provision of a binding called [member binding_name] with a fixed [member value]. Create it with
## [method ProvisionSpec.binding].

## Name of the provided binding.
var binding_name: String
## Value of the provided binding.
var value: Variant


## Creates the provision for binding [param name] with value [param val].
func _init(name: String, val: Variant) -> void:
	binding_name = name
	value = val


## Serialises this provision for the Rust bridge.
func to_bridge_dict() -> Dictionary:
	return {
		"kind": "binding",
		"binding_name": binding_name,
		"value": value,
	}
