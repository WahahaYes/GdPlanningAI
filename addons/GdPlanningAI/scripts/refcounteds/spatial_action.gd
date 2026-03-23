class_name SpatialAction
extends NavigatingAction
## Spatial actions are related to a physical object and contingent on proximity.  Moving the agent
## to the object is bundled into the action.  This class of actions uses Godot's navigation
## to test proximity, and relies on the GdPAI agent having a child NavigationAgent(2D/3D).
##[br]
##[br]
## NOTE: The agent still needs to move itself; this action just updates the navigation target of
## the agent's NavigationAgent.

## Reference to the GdPAI location data that this action is tied to.  This is set when the action
## is created.
var object_location: GdPAILocationData
## Reference to the GdPAI interactable attributes.  This is set when the action is created.
var interactable_attribs: GdPAIInteractable


# Override
func _init(
		p_object_location: GdPAILocationData,
		p_interactable_attribs: GdPAIInteractable,
) -> void:
	self.object_location = p_object_location
	self.interactable_attribs = p_interactable_attribs


# Override
func get_action_cost(
		agent_blackboard: GdPAIBlackboard,
		world_state: GdPAIBlackboard,
) -> float:
	var agent_location: SimObjectProxy = agent_blackboard.get_proxy_in_group("GdPAILocationData")
	if not is_instance_valid(object_location):
		return INF
	var sim_location: SimObjectProxy = world_state.get_object_for(object_location)
	if sim_location == null:
		return INF
	# NOTE: This is a heuristic using Euclidean distance but not taking navigation obstacles into
	# 		account.  Using the navigation agent would be more expensive but yield a more accurate
	# 		cost.
	# TODO: Make navigation agent-based cost an option.  Maybe could configure in plugin.cfg?
	# 		Alternative would be to parameterize within the agent, but that could be tricky.
	var dist: float = (agent_location.get_property("position") - sim_location.get_property("position")).length()
	return dist


# Override
func get_validity_checks() -> Array[Precondition]:
	var checks: Array[Precondition] = []
	checks.append(Precondition.agent_has_property("entity"))
	checks.append(Precondition.agent_has_object_data_of_group("GdPAILocationData"))
	checks.append(Precondition.check_is_object_valid(object_location))
	checks.append(Precondition.check_is_object_valid(interactable_attribs))

	# Can agent get to the target check
	checks.append(Precondition.custom(
		func(
			blackboard: GdPAIBlackboard,
			_world_state: GdPAIBlackboard
		) -> bool:
		var entity: Node = blackboard.get_property("entity")
		if entity == null:
			return false

		var nav_agent: Node = _find_nav_agent(entity)
		if nav_agent == null:
			return false

		# Optionally setting the interaction distance <= 0 bypasses the can_get_to constraint.
		if interactable_attribs.max_interaction_distance <= 0:
			return true

		# Override the entity's nav agent to test if it is possible to get to this object.
		var old_target_position = nav_agent.target_position
		nav_agent.target_position = object_location.position
		nav_agent.get_next_path_position() # Compute the path.
		var final_dist: float = (
			(object_location.position - nav_agent.get_final_position()).length()
		)
		# Restore the nav agent's earlier state.
		nav_agent.target_position = old_target_position
		nav_agent.get_next_path_position()
		return final_dist < interactable_attribs.max_interaction_distance
	))

	return checks


# Override
func simulate_effect(
		agent_blackboard: GdPAIBlackboard,
		world_state: GdPAIBlackboard,
) -> void:
	# Simulate by teleporting the agent to the object's location.
	var agent_location: SimObjectProxy = agent_blackboard.get_proxy_in_group("GdPAILocationData")
	var sim_location: SimObjectProxy = world_state.get_object_for(object_location)
	agent_location.set_property("position", sim_location.get_property("position"))


# Override
func pre_perform_action(agent: GdPAIAgent) -> Action.Status:
	# Failure state in the case the target has been freed since planning.
	if not is_instance_valid(object_location) or not is_instance_valid(interactable_attribs):
		return Action.Status.FAILURE

	var entity: Node = agent.entity

	# Cache the location data.
	var agent_location_data: GdPAILocationData = agent.blackboard.get_node_in_group(
		"GdPAILocationData",
	)
	set_state(agent, "agent_location", agent_location_data)

	var nav_agent: Node = _find_nav_agent(entity)
	assert(nav_agent != null)
	var dist_check: float = (
		ARRIVAL_THRESHOLD_2D if nav_agent is NavigationAgent2D else ARRIVAL_THRESHOLD_3D
	)
	set_state(agent, "nav_agent", nav_agent)
	set_state(agent, "dist_check", dist_check)

	# Set up some flags for movement.
	set_state(agent, "time_elapsed", 0)
	set_state(agent, "target_set", false)
	set_state(agent, "target_reached", false)
	set_state(agent, "object_orig_position", object_location.position)
	set_state(agent, "prior_positions", [agent_location_data.position])

	return Action.Status.SUCCESS


# Override
func perform_action(
		agent: GdPAIAgent,
		delta: float,
) -> Action.Status:
	# Failure state in the case the target has been freed.
	if not is_instance_valid(object_location) or not is_instance_valid(interactable_attribs):
		return Action.Status.FAILURE

	# Fail if the target object has moved too far from its planning-time position.
	# NOTE: These locations are purposefully not typed to be 2D and 3D compatible.
	var orig_position = get_state(agent, "object_orig_position")
	var current_position = object_location.position
	if interactable_attribs.max_drift_from_plan >= 0:
		if (current_position - orig_position).length() > interactable_attribs.max_drift_from_plan:
			return Action.Status.FAILURE

	var nav_agent: Node = get_state(agent, "nav_agent")
	var agent_location_data: GdPAILocationData = get_state(agent, "agent_location")

	# Maintain a list of prior positions to check if the agent isn't moving.
	var prior_positions: Array = get_state(agent, "prior_positions")
	prior_positions.append(agent_location_data.position)
	if prior_positions.size() > 60:
		prior_positions.pop_front()
	set_state(agent, "prior_positions", prior_positions)

	# Keep track of how long we've been actively pursuing this object.
	var time_elapsed: float = get_state(agent, "time_elapsed")
	time_elapsed += delta
	set_state(agent, "time_elapsed", time_elapsed)

	# Begin walking to the target on the first action frame.
	if not get_state(agent, "target_set"):
		nav_agent.target_position = object_location.position
		set_state(agent, "target_set", true)
	# Update the nav agent target if the object has moved too far from its planning-time position.
	elif (
		(nav_agent.target_position - object_location.position).length() >
		interactable_attribs.max_interaction_distance
	):
		nav_agent.target_position = object_location.position

	# Terminating conditions.
	# Either the navigation agent passes, or the agent has stopped for some other reason.
	var dist_traveled: float = (
		(prior_positions[-1] - prior_positions[0]).length() * delta * prior_positions.size()
	)
	var dist_check: float = get_state(agent, "dist_check")
	if (
		nav_agent.is_navigation_finished() or
		(prior_positions.size() == 60 and dist_traveled < dist_check)
	):
		# Pass if we have no interaction distance constraint.
		if interactable_attribs.max_interaction_distance <= 0:
			set_state(agent, "target_reached", true)
			return Action.Status.SUCCESS
		# Else, figure out the final distance and see if valid.
		var final_dist: float = (object_location.position - nav_agent.get_final_position()).length()
		if final_dist < interactable_attribs.max_interaction_distance:
			set_state(agent, "target_reached", true)
			return Action.Status.SUCCESS
		return Action.Status.FAILURE

	# Continue navigating.
	return Action.Status.RUNNING


# Override
func post_perform_action(agent: GdPAIAgent) -> Action.Status:
	# Corresponding failure state to what could skip pre actions.
	if not is_instance_valid(object_location) or not is_instance_valid(interactable_attribs):
		return Action.Status.FAILURE

	var nav_agent: Node = get_state(agent, "nav_agent")
	var agent_location_data: GdPAILocationData = get_state(agent, "agent_location")
	# Clear the navigation target.
	nav_agent.target_position = agent_location_data.position

	erase_state(agent, "nav_agent")
	erase_state(agent, "agent_location")
	erase_state(agent, "target_set")
	erase_state(agent, "target_reached")
	erase_state(agent, "time_elapsed")
	erase_state(agent, "prior_positions")
	erase_state(agent, "object_orig_position")

	return Action.Status.SUCCESS


# Override
func get_title() -> String:
	return "Move To"


# Override
func get_description() -> String:
	return "Move to a target object."
