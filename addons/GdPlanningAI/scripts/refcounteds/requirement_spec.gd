class_name RequirementSpec
extends RefCounted
## A symbolic dependency of an action: something an earlier action in the plan must provide.
## Returned from [method Action.get_requirements].[br]
##[br]
## Requirements are matched by name and kind against the [ProvisionSpec]s of other actions, with
## no simulation involved. A requirement that no action in the chain satisfies makes the action
## unusable. Create requirements with the static constructors below, never by instancing this
## class directly.
## [codeblock]
## func get_requirements() -> Array[RequirementSpec]:
##     return [
##         RequirementSpec.fact("at_target", [location_data]),
##         RequirementSpec.binding_exists("held_item"),
##     ]
## [/codeblock]
## See also [ProvisionSpec] for the providing side.


## Returns the dictionary the Rust planning engine reads this requirement from. Called by the
## bridge on the main thread when a plan request is built. Implemented by the subclasses; user
## code does not call or override it.
func to_bridge_dict() -> Dictionary:
	push_error("RequirementSpec.to_bridge_dict must be overridden")
	return {}


## Requires that some earlier action provides a binding called [param name], whatever its value
## (matched by [method ProvisionSpec.binding]).
static func binding_exists(name: String) -> RequirementSpec:
	return RequirementSpecBindingExists.new(name)


## Requires that some earlier action provides a binding called [param name] with exactly
## [param value].
static func binding_equals(name: String, value: Variant) -> RequirementSpec:
	return RequirementSpecBindingEquals.new(name, value)


## Requires a binding called [param name] whose value is an object belonging to the group
## [param set_name].
static func binding_in_set(name: String, set_name: String) -> RequirementSpec:
	return RequirementSpecBindingInSet.new(name, set_name)


## Requires that an earlier action provides the fact [param name] with the same [param args]
## (matched by [method ProvisionSpec.fact]), or a wildcard provision of the same name
## ([method ProvisionSpec.fact_wildcard]), which takes [param args] as its binding.
static func fact(name: String, args: Array[Variant] = []) -> RequirementSpec:
	return RequirementSpecFact.new(name, args)
