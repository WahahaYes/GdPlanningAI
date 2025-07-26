class_name SampleWanderAction
extends Action

var wander_distance: float


# This action shows how you can directly inject parameters.
func _init(wander_distance: float):
	# If implementing _init(), make sure to call super() so a uid is created.
	super()
	self.wander_distance = wander_distance


# Override
func get_validity_checks() -> Array[Precondition]:
	var checks: Array[Precondition] = []
	checks.append(Precondition.agent_has_property("entity"))
	checks.append(Precondition.agent_has_object_data_of_group("GdPAILocationData"))
	return checks


# Override
func get_action_cost(agent_blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard) -> float:
	return wander_distance


# Override
func get_preconditions() -> Array[Precondition]:
	return []


# Override
func simulate_effect(agent_blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):
	var sim_location: GdPAILocationData = agent_blackboard.get_first_object_in_group(
		"GdPAILocationData"
	)
	sim_location.position.x += wander_distance


# Override
func reverse_simulate_effect(agent_blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):
	pass


# Override
func pre_perform_action(agent: GdPAIAgent) -> Action.Status:
	# Cache location data.
	var location_data: GdPAILocationData = agent.blackboard.get_first_object_in_group(
		"GdPAILocationData"
	)
	agent.blackboard.set_property(uid_property("agent_location"), location_data)

	var entity: Node = agent.blackboard.get_property("entity")
	# Parse through 2D and 3D case to determine nav_agent and target_location.
	var nav_agent: Node
	var random_dir: Vector2 = Vector2.from_angle(deg_to_rad(randf_range(-180, 180)))
	if location_data.location_node_2d != null:
		nav_agent = GdPAIUTILS.get_child_of_type(entity, NavigationAgent2D)
		var target_location: Vector2 = location_data.position + random_dir * wander_distance
		agent.blackboard.set_property(uid_property("target_location"), target_location)
	elif location_data.location_node_3d != null:
		nav_agent = GdPAIUTILS.get_child_of_type(entity, NavigationAgent3D)
		var random_dir_3d: Vector3 = Vector3(random_dir.x, 0, random_dir.y)
		var target_location: Vector3 = location_data.position + random_dir_3d * wander_distance
		agent.blackboard.set_property(uid_property("target_location"), target_location)

	# Set up movement flags.
	agent.blackboard.set_property(uid_property("nav_agent"), nav_agent)
	agent.blackboard.set_property(uid_property("target_set"), false)
	agent.blackboard.set_property(uid_property("prior_positions"), [location_data.position])
	return Action.Status.SUCCESS


# Override
func perform_action(agent: GdPAIAgent, delta: float) -> Action.Status:
	# Grab needed properties from blackboard.
	var nav_agent: Node = agent.blackboard.get_property(uid_property("nav_agent"))
	var agent_location_data: GdPAILocationData = agent.blackboard.get_property(
		uid_property("agent_location")
	)
	var target_location = agent.blackboard.get_property(uid_property("target_location"))

	# Maintain a list of 10 prior positions.
	var prior_positions: Array = agent.blackboard.get_property(uid_property("prior_positions"))
	prior_positions.append(agent_location_data.position)
	if prior_positions.size() > 60:
		prior_positions.pop_front()
	agent.blackboard.set_property(uid_property("prior_positions"), prior_positions)

	# Begin walking to the target on the first action frame.
	if not agent.blackboard.get_property(uid_property("target_set")):
		nav_agent.target_position = target_location
		agent.blackboard.set_property(uid_property("target_set"), true)

	# Terminating conditions.
	# Either the navigation agent passes, or the agent has stopped for some other reason.
	var dist_traveled: float = (
		(prior_positions[-1] - prior_positions[0]).length() * delta * prior_positions.size()
	)
	if nav_agent.is_navigation_finished() or (prior_positions.size() == 60 and dist_traveled < 1):
		return Action.Status.SUCCESS

	# Continue navigating.
	return Action.Status.RUNNING


# Override
func post_perform_action(agent: GdPAIAgent) -> Action.Status:
	var nav_agent: Node = agent.blackboard.get_property(uid_property("nav_agent"))
	var agent_location_data: GdPAILocationData = agent.blackboard.get_property(
		uid_property("agent_location")
	)
	# Clear the navigation target.
	nav_agent.target_position = agent_location_data.position

	agent.blackboard.erase_property(uid_property("nav_agent"))
	agent.blackboard.erase_property(uid_property("agent_location"))
	agent.blackboard.erase_property(uid_property("target_location"))
	agent.blackboard.erase_property(uid_property("target_set"))
	agent.blackboard.erase_property(uid_property("prior_positions"))

	return Action.Status.SUCCESS
