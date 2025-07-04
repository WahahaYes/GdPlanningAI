## Blackboard Plan allows for defining the start point for an agent blackboard or world state.
class_name GdPAIBlackboardPlan
extends Resource

## The initial blackboard contents to define.
@export var blackboard_backend: Dictionary


## Generates a Blackboard instance using the dictionary specified by the BlackboardPlan
func generate_blackboard() -> GdPAIBlackboard:
	var gen_blackboard = GdPAIBlackboard.new()
	gen_blackboard._blackboard = blackboard_backend.duplicate(true)
	if GdPAIBlackboard.GdPAI_OBJECTS not in gen_blackboard._blackboard:
		gen_blackboard._blackboard[GdPAIBlackboard.GdPAI_OBJECTS] = []
	return gen_blackboard
