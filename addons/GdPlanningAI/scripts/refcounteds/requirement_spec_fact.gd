class_name RequirementSpecFact
extends RequirementSpec
## Requires that a fact with specific arguments exists.

var fact_name: String
var args: Array[Variant] = []


func _init(name: String, fact_args: Array[Variant] = []) -> void:
	fact_name = name
	args.assign(fact_args)


func to_bridge_dict() -> Dictionary:
	return {
		"kind": "fact",
		"fact_name": fact_name,
		"args": args,
	}
