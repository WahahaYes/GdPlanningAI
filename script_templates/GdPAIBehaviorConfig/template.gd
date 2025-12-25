# meta-description: GdPAI BehaviorConfig template.
# meta-default: true
extends GdPAIBehaviorConfig

# Add your configurable properties here with @export for serialization.
@export var example_parameter: float = 1.0


# Override _self_init to set up goals and actions after @export values are applied.
func _self_init():
	super()
	# Configure goals, self_actions, and property_updaters here.
	# You can pass @export properties to your custom classes.
	goals.append(Goal.new())
	self_actions.append(Action.new())
	property_updaters.append(TemplatePropertyUpdater.new(example_parameter))
