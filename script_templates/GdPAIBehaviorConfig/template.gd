# meta-description: GdPAI BehaviorConfig template.
# meta-default: true
extends GdPAIBehaviorConfig

## Example configurable parameter.
@export var example_parameter: float = 1.0


# Override
func _populate(
	goals: Array[Goal],
	actions: Array[Action],
	updaters: Array[PropertyUpdater],
) -> void:
	# Append new instances; this runs once per agent, so never reuse objects across calls.
	goals.append(Goal.new())
	actions.append(Action.new())
	updaters.append(PropertyUpdater.new())
