extends Node
## Simple animation script used to power the GdPAI 2D demo.

## Reference to the agent's entity
@export var entity: RigidBody2D
## Reference to the sprite.
@export var animated_sprite: AnimatedSprite2D
## Velocity threshold for sprite animation.
@export var idle_threshold: float = 1
## Reference to the GdPAI agent.
@export var gdpai_agent: GdPAIAgent
## Reference to a textual display which shows some attributes of the agent.
@export var display_text: Label


func _process(delta: float) -> void:
	# Animations.
	if entity.linear_velocity.length() > idle_threshold:
		animated_sprite.play("Run")
	else:
		animated_sprite.play("Idle")
	# Flip the agent to face their movement direction.
	if entity.linear_velocity.x < 0:
		animated_sprite.scale.x = -1
	elif entity.linear_velocity.x > 0:
		animated_sprite.scale.x = 1

	# Hacky way to display the goal and action being considered at this moment.
	var hunger: float = gdpai_agent.blackboard.get_property("hunger")
	var goal_text: String
	if gdpai_agent._current_goal != null:
		goal_text = gdpai_agent._current_goal.get_title()

	var action_text: String
	if gdpai_agent._current_plan != null and gdpai_agent._current_plan.get_plan().size() > 0:
		var step: int = min(
			gdpai_agent._current_plan_step,
			gdpai_agent._current_plan.get_plan().size() - 1,
		)
		var action: Action = gdpai_agent._current_plan.get_plan()[step]
		action_text = action.get_title()

	display_text.text = (
		"Hunger: %.f\nGoal: %s\nCurrent\nAction: %s" % [hunger, goal_text, action_text]
	)
