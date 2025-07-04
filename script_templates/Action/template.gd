# meta-description: GdPAI Action template.
# meta-default: true

extends Action


func _init():
	# If implementing _init(), make sure to call super() so a uid is created.
	super()


# Override
func get_validity_checks() -> Array[Precondition]:
	return []


# Override
func get_action_cost(agent_blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard) -> float:
	return 0


# Override
func get_preconditions() -> Array[Precondition]:
	return []


# Override
func simulate_effect(agent_blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):
	pass


# Override
func reverse_simulate_effect(agent_blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):
	pass


# Override
func pre_perform_action(agent: GdPAIAgent) -> Action.Status:
	return Action.Status.SUCCESS


# Override
func perform_action(agent: GdPAIAgent, delta: float) -> Action.Status:
	return Action.Status.SUCCESS


# Override
func post_perform_action(agent: GdPAIAgent) -> Action.Status:
	return Action.Status.SUCCESS


# Override
func copy_for_simulation() -> Action:
	# Override if copying object data over.  Otherwise, no need to.
	# Make sure to replace <Action> with the subclass name, and to duplicate any new properties.
	var dupe: Action = Action.new()
	return dupe
