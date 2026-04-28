class_name RequirementSpec
extends RefCounted


func to_bridge_dict() -> Dictionary:
	push_error("RequirementSpec.to_bridge_dict must be overridden")
	return {}


static func binding_exists(name: String) -> RequirementSpec:
	return RequirementSpecBindingExists.new(name)


static func binding_equals(name: String, value: Variant) -> RequirementSpec:
	return RequirementSpecBindingEquals.new(name, value)


static func binding_in_set(name: String, set_name: String) -> RequirementSpec:
	return RequirementSpecBindingInSet.new(name, set_name)


static func fact(name: String, args: Array[Variant] = []) -> RequirementSpec:
	return RequirementSpecFact.new(name, args)
