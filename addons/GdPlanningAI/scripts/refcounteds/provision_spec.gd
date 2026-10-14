class_name ProvisionSpec
extends RefCounted
## Something an action offers to later actions in the plan. Returned from
## [method Action.get_provisions].[br]
##[br]
## Provisions are matched against the [RequirementSpec]s of other actions by name and kind, with
## no simulation involved. The planner inserts a providing action before the earliest action that
## needs it. Create provisions with the static constructors below, never by instancing this class
## directly.
## [codeblock]
## func get_provisions() -> Array[ProvisionSpec]:
##     return [ProvisionSpec.binding("held_item", "wood"), ProvisionSpec.fact("is_food")]
## [/codeblock]
## See also [RequirementSpec] for the consuming side.


## Returns the dictionary the Rust planning engine reads this provision from. Called by the
## bridge on the main thread when a plan request is built. Implemented by the subclasses; user
## code does not call or override it.
func to_bridge_dict() -> Dictionary:
	push_error("ProvisionSpec.to_bridge_dict must be overridden")
	return {}


## Provides a binding called [param name] with [param value]. Satisfies
## [method RequirementSpec.binding_exists], [method RequirementSpec.binding_equals] (when the
## value matches) and [method RequirementSpec.binding_in_set] (when the value is an object in the
## group).
static func binding(name: String, value: Variant) -> ProvisionSpec:
	return ProvisionSpecBinding.new(name, value)


## Provides the fact [param name] with [param args]. Satisfies a [method RequirementSpec.fact]
## with the same name and arguments.
static func fact(name: String, args: Array[Variant] = []) -> ProvisionSpec:
	return ProvisionSpecFact.new(name, args)


## Provides the fact [param name] for any arguments: it satisfies every
## [method RequirementSpec.fact] of that name, and the requirement's arguments become the
## concrete binding passed to the action's [code]inject_binding[/code] before execution.
## [GoToAction] uses it for [code]at_target[/code].
static func fact_wildcard(name: String) -> ProvisionSpec:
	return ProvisionSpecFactWildcard.new(name)
