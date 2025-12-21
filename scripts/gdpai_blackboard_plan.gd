class_name GdPAIBlackboardPlan
extends Resource
## Blackboard Plan allows for defining the start point for an agent blackboard or world state.

## The initial blackboard contents to define.
@export var blackboard_backend: Dictionary


## Generates a Blackboard instance using the dictionary specified by the BlackboardPlan
func generate_blackboard() -> GdPAIBlackboard:
	var gen_blackboard = GdPAIBlackboard.new()
	gen_blackboard.set_dict(blackboard_backend.duplicate(true))
	if GdPAIBlackboard.GDPAI_OBJECTS not in gen_blackboard.get_dict():
		gen_blackboard._blackboard[GdPAIBlackboard.GDPAI_OBJECTS] = []
	return gen_blackboard
