class_name GdPAIBlackboardPlan
extends Resource
## Resource describing the starting contents of a blackboard. It is used by
## [member GdPAIAgentConfig.blackboard_plan] for an agent's own blackboard and by
## [member GdPAIWorldNode.blackboard_plan] for the world state.[br]
##[br]
## Edit [member blackboard_backend] in the inspector with the initial properties (for example
## [code]hunger = 0.0[/code]). [method generate_blackboard] turns it into a fresh live
## [GdPAIBlackboard] each time it is called, so one plan can safely be shared by many agents.

## Initial property names and values copied into each generated blackboard.
## The key [code]GDPAI_OBJECTS[/code] is reserved for the addon.
@export var blackboard_backend: Dictionary


## Returns a new [GdPAIBlackboard] filled with a deep copy of [member blackboard_backend]. Adds an
## empty [code]GDPAI_OBJECTS[/code] list if the dictionary does not contain one. Modifying the
## result never changes the plan.
func generate_blackboard() -> GdPAIBlackboard:
	var gen_blackboard = GdPAIBlackboard.new()
	gen_blackboard.set_dict(blackboard_backend.duplicate(true))
	if not gen_blackboard.has_property("GDPAI_OBJECTS"):
		gen_blackboard.set_property("GDPAI_OBJECTS", [])
	return gen_blackboard
