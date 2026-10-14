class_name RequirementSpec
extends RefCounted
## Something an earlier action in the plan must provide, matched by name against its
## [ProvisionSpec]s. Returned from [method Action.get_requirements]; create one with the static
## constructors.


## Returns the engine's view of this requirement. Implemented by subclasses; internal.
func to_bridge_dict() -> Dictionary:
	push_error("RequirementSpec.to_bridge_dict must be overridden")
	return {}


## Requires a binding [param name] with any value.
static func binding_exists(name: String) -> RequirementSpec:
	return RequirementSpecBindingExists.new(name)


## Requires a binding [param name] equal to [param value].
static func binding_equals(name: String, value: Variant) -> RequirementSpec:
	return RequirementSpecBindingEquals.new(name, value)


## Requires a binding [param name] whose value is an object in group [param set_name].
static func binding_in_set(name: String, set_name: String) -> RequirementSpec:
	return RequirementSpecBindingInSet.new(name, set_name)


## Requires the fact [param name] with [param args], or a wildcard provision of that name.
static func fact(name: String, args: Array[Variant] = []) -> RequirementSpec:
	return RequirementSpecFact.new(name, args)
