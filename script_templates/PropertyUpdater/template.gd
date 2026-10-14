# meta-description: GdPAI PropertyUpdater template.
# meta-default: true
extends PropertyUpdater

## Example parameter, passed in by the behavior config that creates this updater.
var example_parameter: float = 1.0


func _init(p_example_parameter: float = 1.0) -> void:
	example_parameter = p_example_parameter


# Override
func initialize(agent: GdPAIAgent) -> void:
	# Called once when the behavior config is applied: create the properties you own.
	agent.blackboard.set_property("example_parameter", example_parameter)


# Override
func update_properties(
	agent: GdPAIAgent,
	delta: float,
) -> void:
	# Called every frame, never during planning.
	var runtime_value: float = agent.blackboard.get_property("example_parameter")
	agent.blackboard.set_property("example_parameter", runtime_value + delta)
