class_name ProvisionSpecFact
extends ProvisionSpec
## Provision of a fact called [member fact_name] with specific [member args]. Create it with
## [method ProvisionSpec.fact].

## Name of the provided fact.
var fact_name: String
## Arguments of the provided fact.
var args: Array[Variant] = []


## Creates the provision for fact [param name] with [param fact_args].
func _init(name: String, fact_args: Array[Variant] = []) -> void:
	fact_name = name
	args.assign(fact_args)


## Serialises this provision for the Rust bridge.
func to_bridge_dict() -> Dictionary:
	return {
		"kind": "fact",
		"fact_name": fact_name,
		"args": args,
	}
