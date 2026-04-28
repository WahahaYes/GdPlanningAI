class_name ProvisionSpec
extends RefCounted


func to_bridge_dict() -> Dictionary:
	push_error("ProvisionSpec.to_bridge_dict must be overridden")
	return {}


static func binding(name: String, value: Variant) -> ProvisionSpec:
	return ProvisionSpecBinding.new(name, value)


static func fact(name: String, args: Array[Variant] = []) -> ProvisionSpec:
	return ProvisionSpecFact.new(name, args)
