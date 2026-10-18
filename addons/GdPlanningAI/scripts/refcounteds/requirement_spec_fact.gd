class_name RequirementSpecFact
extends RequirementSpec
## Requirement that a fact called [member fact_name] with the given [member args] was provided by
## an earlier action. Create it with [method RequirementSpec.fact].

## Name of the required fact.
var fact_name: String
## Arguments of the required fact.
var args: Array[Variant] = []


## Creates the requirement for fact [param name] with [param fact_args].
func _init(name: String, fact_args: Array[Variant] = []) -> void:
	fact_name = name
	args.assign(fact_args)


## Serialises this requirement for the Rust bridge.
func to_bridge_dict() -> Dictionary:
	return {
		"kind": "fact",
		"fact_name": fact_name,
		"args": args,
	}
