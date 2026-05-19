# meta-description: GdPAI PropertyUpdater template.
# meta-default: true
class_name TemplatePropertyUpdater
extends PropertyUpdater

## Add your configurable properties here.  They can be passed in from
## the GdPAIBehaviorConfig that instantiates the PropertyUpdater.
var example_parameter: float = 1.0


func _init(p_example_parameter: float) -> void:
	# Assign any parameters here.
	self.example_parameter = p_example_parameter


# Override
func initialize(agent: GdPAIAgent) -> void:
	# Set up initial property values with the agent.
	agent.blackboard.set_property("example_parameter", example_parameter)


# Override
func update_properties(
		agent: GdPAIAgent,
		delta: float,
) -> void:
	# Update agent properties that need to be maintained as part of this behavior.
	var runtime_value: float = agent.blackboard.get_property("example_parameter")
	agent.blackboard.set_property("example_parameter", runtime_value + delta)
