class_name RequirementSpecBindingEquals
extends RequirementSpec
## Requirement that a binding called [member binding_name] was provided with exactly [member value].
## Create it with [method RequirementSpec.binding_equals].

## Name of the required binding.
var binding_name: String
## The exact value the binding must have.
var value: Variant


## Creates the requirement for binding [param name] equal to [param val].
func _init(name: String, val: Variant) -> void:
	binding_name = name
	value = val


## Serialises this requirement for the Rust bridge.
func to_bridge_dict() -> Dictionary:
	return {
		"kind": "binding_equals",
		"binding_name": binding_name,
		"value": value,
	}
