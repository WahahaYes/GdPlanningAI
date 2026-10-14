class_name GdPAIBlackboardPlan
extends Resource
## Starting contents of a blackboard, for an agent ([member GdPAIAgentConfig.blackboard_plan]) or
## the world ([member GdPAIWorldNode.blackboard_plan]).

## Initial property names and values copied into each generated blackboard.
## The key [code]GDPAI_OBJECTS[/code] is reserved for the addon.
@export var blackboard_backend: Dictionary


## Returns a new [GdPAIBlackboard] holding a deep copy of [member blackboard_backend].
func generate_blackboard() -> GdPAIBlackboard:
	var gen_blackboard = GdPAIBlackboard.new()
	gen_blackboard.set_dict(blackboard_backend.duplicate(true))
	if not gen_blackboard.has_property("GDPAI_OBJECTS"):
		gen_blackboard.set_property("GDPAI_OBJECTS", [])
	return gen_blackboard
