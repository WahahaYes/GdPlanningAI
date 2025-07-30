# meta-description: GdPAI SpatialAction template.
# meta-default: true

extends SpatialAction


# Override
func _init(object_location: GdPAILocationData, interactable_attribs: GdPAIInteractable):
	# If extending _init(), make sure to call super() so a uid is created and references are
	# assigned.
	super(object_location, interactable_attribs)


# Override
func get_validity_checks() -> Array[Precondition]:
	var checks: Array[Precondition] = super()
	# Add any additional checks here.
	return checks


# Override
func get_action_cost(agent_blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard) -> float:
	var cost: float = super(agent_blackboard, world_state)
	if cost == INF:
		return INF
	# Add any additional cost computations.
	return cost


# Override
func get_preconditions() -> Array[Precondition]:
	return []


# Override
func simulate_effect(agent_blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):
	super(agent_blackboard, world_state)
	# Add any additional simulation here.


# Override
func reverse_simulate_effect(agent_blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):
	pass


# Override
func pre_perform_action(agent: GdPAIAgent) -> Action.Status:
	if super(agent) == Action.Status.FAILURE:
		return Action.Status.FAILURE
	# Add any additional preactions here.
	return Action.Status.SUCCESS


# Override
func perform_action(agent: GdPAIAgent, delta: float) -> Action.Status:
	var parent_status: Action.Status = super(agent, delta)
	if parent_status == Action.Status.FAILURE:
		return Action.Status.FAILURE
	if not agent.blackboard.get_property(uid_property("target_reached")):
		# Can add any actions that occur while navigating here.
		return Action.Status.RUNNING
	else:
		# Add the main action that occurs after the agent navigates to the object here.
		return Action.Status.SUCCESS


# Override
func post_perform_action(agent: GdPAIAgent) -> Action.Status:
	super(agent)
	# Add any additional postactions here.
	return Action.Status.SUCCESS
