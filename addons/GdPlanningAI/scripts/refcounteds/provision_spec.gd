class_name ProvisionSpec
extends RefCounted
## Something an action offers later actions, matched by name against their [RequirementSpec]s.
## Returned from [method Action.get_provisions]; create one with the static constructors.


## Returns the engine's view of this provision. Implemented by subclasses; internal.
func to_bridge_dict() -> Dictionary:
	push_error("ProvisionSpec.to_bridge_dict must be overridden")
	return {}


## Provides a binding [param name] = [param value].
static func binding(name: String, value: Variant) -> ProvisionSpec:
	return ProvisionSpecBinding.new(name, value)


## Provides the fact [param name] with [param args].
static func fact(name: String, args: Array[Variant] = []) -> ProvisionSpec:
	return ProvisionSpecFact.new(name, args)


## Provides the fact [param name] for any arguments; the requirement's arguments are passed to
## the action's [code]inject_binding[/code]. [GoToAction] uses it for [code]at_target[/code].
static func fact_wildcard(name: String) -> ProvisionSpec:
	return ProvisionSpecFactWildcard.new(name)
