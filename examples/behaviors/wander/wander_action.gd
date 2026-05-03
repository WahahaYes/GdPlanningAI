class_name WanderAction
extends Action
## Navigates the agent to a random nearby point.
##[br]
##[br]
## Demonstrates a [SpatialAction] sibling that has no specific world target — the
## destination is computed at execution time from the agent's current position.

## How far from the agent's current position the wander target may be.
var wander_distance: float


func _init(p_wander_distance: float) -> void:
	wander_distance = p_wander_distance


# Override
func get_validity_checks() -> Array[Precondition]:
	var checks: Array[Precondition] = []
	checks.append(Precondition.agent_has_property("entity"))
	checks.append(Precondition.agent_has_object_data_of_group("GdPAILocationData"))
	return checks


# Override
func get_action_cost(
	_agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> float:
	return 0.0


# Override
func get_preconditions() -> Array[Precondition]:
	return []


# Override
func simulate_effect(
	agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> void:
	var sim_location: SimObjectProxy = agent_blackboard.get_proxy_in_group("GdPAILocationData")
	var current_pos: Variant = sim_location.get_property("position")
	if current_pos is Vector2:
		sim_location.set_property("position", current_pos + Vector2(wander_distance, 0))
	elif current_pos is Vector3:
		sim_location.set_property("position", current_pos + Vector3(wander_distance, 0, 0))


# Override
func pre_perform_action(agent: GdPAIAgent) -> Action.Status:
	var location_data: GdPAILocationData = (
		agent
		. blackboard
		. get_node_in_group(
			"GdPAILocationData",
		)
	)
	set_state(agent, "agent_location", location_data)

	var entity: Node = agent.entity
	var nav_agent: Node = SpatialAction.find_nav_agent(entity)

	var random_dir: Vector2 = Vector2.from_angle(deg_to_rad(randf_range(-180, 180)))
	if nav_agent is NavigationAgent2D:
		var target_location: Vector2 = location_data.position + random_dir * wander_distance
		set_state(agent, "target_location", target_location)
	else:
		var random_dir_3d: Vector3 = Vector3(random_dir.x, 0, random_dir.y)
		var target_location: Vector3 = location_data.position + random_dir_3d * wander_distance
		set_state(agent, "target_location", target_location)

	set_state(agent, "nav_agent", nav_agent)
	set_state(agent, "target_set", false)
	set_state(agent, "prior_positions", [location_data.position])
	return Action.Status.SUCCESS


# Override
func perform_action(agent: GdPAIAgent, delta: float) -> Action.Status:
	var nav_agent: Node = get_state(agent, "nav_agent")
	var agent_location_data: GdPAILocationData = get_state(agent, "agent_location")
	var target_location: Variant = get_state(agent, "target_location")

	var prior_positions: Array = get_state(agent, "prior_positions")
	prior_positions.append(agent_location_data.position)
	if prior_positions.size() > 60:
		prior_positions.pop_front()
	set_state(agent, "prior_positions", prior_positions)

	if not get_state(agent, "target_set"):
		nav_agent.target_position = target_location
		set_state(agent, "target_set", true)

	var dist_traveled: float = (
		(prior_positions[-1] - prior_positions[0]).length() * delta * prior_positions.size()
	)
	var dist_check: float = (
		SpatialAction.ARRIVAL_THRESHOLD_2D
		if nav_agent is NavigationAgent2D
		else SpatialAction.ARRIVAL_THRESHOLD_3D
	)
	if (
		nav_agent.is_navigation_finished()
		or (prior_positions.size() == 60 and dist_traveled < dist_check)
	):
		return Action.Status.SUCCESS

	return Action.Status.RUNNING


# Override
func post_perform_action(agent: GdPAIAgent) -> Action.Status:
	var nav_agent: Node = get_state(agent, "nav_agent")
	var agent_location_data: GdPAILocationData = get_state(agent, "agent_location")
	nav_agent.target_position = agent_location_data.position

	erase_state(agent, "nav_agent")
	erase_state(agent, "agent_location")
	erase_state(agent, "target_location")
	erase_state(agent, "target_set")
	erase_state(agent, "prior_positions")
	return Action.Status.SUCCESS


# Override
func get_title() -> String:
	return "Wander"


# Override
func get_description() -> String:
	return "Navigate to a random nearby point."
