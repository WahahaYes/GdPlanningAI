# meta-description: GdPAI BehaviorConfig template.
# meta-default: true
extends GdPAIBehaviorConfig

# Add your configurable properties here with @export for serialization.
## Example configurable parameter.
@export var example_parameter: float = 1.0


# Override _populate to add goals, actions, and property updaters.
func _populate(
	goals: Array[Goal],
	actions: Array[Action],
	updaters: Array[PropertyUpdater],
) -> void:
	# Append to the provided arrays. You can use the @export properties you defined here.
	goals.append(Goal.new())
	actions.append(Action.new())
	updaters.append(PropertyUpdater.new())
