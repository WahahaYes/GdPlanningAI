class_name GoToAction
extends Action
## A generic navigation action provided by the agent.
## This action provides a wildcard `at_target` provision that can satisfy
## any interaction action requiring the agent to be at a specific location.

## Arrival distance threshold for 2D navigation (pixels).
const ARRIVAL_THRESHOLD_2D: float = 8.0
## Arrival distance threshold for 3D navigation (meters).
const ARRIVAL_THRESHOLD_3D: float = 0.1

## The target location to navigate to. This will be bound during planning
## when the action is selected to satisfy an interaction's requirement.
var target_location: GdPAILocationData


# Override
func _init(p_target_location: GdPAILocationData = null) -> void:
	target_location = p_target_location


# Override
func get_action_cost(
	agent_blackboard: GdPAIBlackboard,
	world_state: GdPAIBlackboard,
) -> float:
	var agent_location: SimObjectProxy = agent_blackboard.get_proxy_in_group("GdPAILocationData")

	# If target_location is set, use it. Otherwise, check for binding from planner.
	var actual_target = target_location
	if actual_target == null:
		var binding = agent_blackboard.get_property("at_target")
		if binding is Array:
			# Binding is an array of locations - choose the first one
			if binding.size() > 0:
				actual_target = binding[0]
		elif binding is GdPAILocationData:
			actual_target = binding
		elif binding is Object:
			# Binding might be passed as ObjectRef
			var obj = instance_from_id(binding)
			if obj is GdPAILocationData:
				actual_target = obj

	if actual_target == null or not is_instance_valid(actual_target):
		return INF
	var sim_location: SimObjectProxy = world_state.get_object_for(actual_target)
	if sim_location == null:
		return INF
	# Euclidean distance heuristic
	var dist: float = (
		(agent_location.get_property("position") - sim_location.get_property("position")).length()
	)
	return dist


# Override
func get_validity_checks() -> Array[Precondition]:
	var checks: Array[Precondition] = []
	checks.append(Precondition.agent_has_property("entity"))
	checks.append(Precondition.agent_has_object_data_of_group("GdPAILocationData"))
	if target_location != null:
		checks.append(Precondition.check_is_object_valid(target_location))
	return checks


# Override
func get_provisions() -> Array[ProvisionSpec]:
	# Provide wildcard at_target fact that can satisfy any at_target requirement
	return [ProvisionSpec.fact_wildcard("at_target")]


# Override
func get_requirements() -> Array[RequirementSpec]:
	return []


# Override
func simulate_effect(
	agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> void:
	# If target_location is set, use it. Otherwise, check for binding from planner.
	var actual_target = target_location
	if actual_target == null:
		var binding = agent_blackboard.get_property("at_target")
		if binding is Array:
			# Binding is an array of locations - choose the first one
			if binding.size() > 0:
				actual_target = binding[0]
		elif binding is GdPAILocationData:
			actual_target = binding

	# Update agent's at_target fact to the target location
	if actual_target != null and is_instance_valid(actual_target):
		agent_blackboard.set_property("at_target", actual_target)
		var agent_location: SimObjectProxy = (
			agent_blackboard.get_proxy_in_group("GdPAILocationData")
		)
		if agent_location != null:
			agent_location.set_property("position", actual_target.position)


# Override
func pre_perform_action(agent: GdPAIAgent) -> Action.Status:
	if target_location == null or not is_instance_valid(target_location):
		return Action.Status.FAILURE

	var entity: Node = agent.entity
	var nav_agent: Node = SpatialAction.find_nav_agent(entity)
	if nav_agent == null:
		return Action.Status.FAILURE

	var dist_check: float = (
		ARRIVAL_THRESHOLD_2D if nav_agent is NavigationAgent2D else ARRIVAL_THRESHOLD_3D
	)

	# Cache the location data.
	var agent_location_data: GdPAILocationData = (
		agent.blackboard.get_node_in_group("GdPAILocationData")
	)

	# Set up state for navigation.
	set_state(agent, "nav_agent", nav_agent)
	set_state(agent, "dist_check", dist_check)
	set_state(agent, "agent_location", agent_location_data)
	set_state(agent, "target_set", false)
	set_state(agent, "target_reached", false)
	set_state(agent, "time_elapsed", 0.0)
	set_state(agent, "prior_positions", [agent_location_data.position])
	set_state(agent, "target_orig_position", target_location.position)

	return Action.Status.SUCCESS


# Override
func perform_action(agent: GdPAIAgent, delta: float) -> Action.Status:
	if target_location == null or not is_instance_valid(target_location):
		return Action.Status.FAILURE

	var nav_agent: Node = get_state(agent, "nav_agent")
	var agent_location_data: GdPAILocationData = get_state(agent, "agent_location")
	var dist_check: float = get_state(agent, "dist_check")
	var target_orig_position = get_state(agent, "target_orig_position")

	# Maintain a list of prior positions to check if the agent isn't moving.
	var prior_positions: Array = get_state(agent, "prior_positions")
	prior_positions.append(agent_location_data.position)
	if prior_positions.size() > 60:
		prior_positions.pop_front()
	set_state(agent, "prior_positions", prior_positions)

	# Keep track of how long we've been actively pursuing this target.
	var time_elapsed: float = get_state(agent, "time_elapsed")
	time_elapsed += delta
	set_state(agent, "time_elapsed", time_elapsed)

	# Begin walking to the target on the first action frame.
	if not get_state(agent, "target_set"):
		nav_agent.target_position = target_location.position
		set_state(agent, "target_set", true)

	# Terminating conditions.
	var dist_traveled: float = (
		(prior_positions[-1] - prior_positions[0]).length() * delta * prior_positions.size()
	)
	if (
		nav_agent.is_navigation_finished()
		or (prior_positions.size() == 60 and dist_traveled < dist_check)
	):
		set_state(agent, "target_reached", true)
		return Action.Status.SUCCESS

	# Continue navigating.
	return Action.Status.RUNNING


# Override
func post_perform_action(agent: GdPAIAgent) -> Action.Status:
	if target_location == null or not is_instance_valid(target_location):
		return Action.Status.FAILURE

	var nav_agent: Node = get_state(agent, "nav_agent")
	var agent_location_data: GdPAILocationData = get_state(agent, "agent_location")

	# Clear the navigation target.
	nav_agent.target_position = agent_location_data.position

	# Clean up state.
	erase_state(agent, "nav_agent")
	erase_state(agent, "dist_check")
	erase_state(agent, "agent_location")
	erase_state(agent, "target_set")
	erase_state(agent, "target_reached")
	erase_state(agent, "time_elapsed")
	erase_state(agent, "prior_positions")
	erase_state(agent, "target_orig_position")

	return Action.Status.SUCCESS
