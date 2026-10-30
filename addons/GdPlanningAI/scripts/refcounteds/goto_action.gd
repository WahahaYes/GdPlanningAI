class_name GoToAction
extends Action
## Walks the agent's entity to a [GdPAILocationData] with its [NavigationAgent2D] or
## [NavigationAgent3D].[br]
## Provides the wildcard fact [code]at_target[/code]: any action requiring
## [code]RequirementSpec.fact("at_target", [location])[/code] gets a GoTo step planned before
## it.[br]
## Succeeds when navigation finishes, or when the agent is stuck (moved less than the arrival
## threshold over [constant STUCK_WINDOW]) within [constant STUCK_ARRIVAL_FACTOR] x
## [code]target_desired_distance[/code] of its path end. Stuck further away, it fails and the agent
## replans.

## Minimum movement (pixels) per [constant STUCK_WINDOW] before a 2D agent counts as stuck.
const ARRIVAL_THRESHOLD_2D: float = 8.0
## Minimum movement (meters) per [constant STUCK_WINDOW] before a 3D agent counts as stuck.
const ARRIVAL_THRESHOLD_3D: float = 0.1
## Seconds of movement history used to detect being stuck.
const STUCK_WINDOW: float = 1.0
## How many [code]target_desired_distance[/code]s from the path end still count as arrived.
const STUCK_ARRIVAL_FACTOR: float = 2.0

## Where to go. Set by [method inject_binding], or fixed through [code]_init[/code].
var target_location: GdPAILocationData


## Returns the navigation agent below [param entity], or [code]null[/code].
static func find_nav_agent(entity: Node) -> Node:
	var nav_2d: Node = GdPAIUTILS.get_child_of_type(entity, NavigationAgent2D)
	var nav_3d: Node = GdPAIUTILS.get_child_of_type(entity, NavigationAgent3D)
	assert(
		nav_2d == null or nav_3d == null,
		"Entity should not have both a NavigationAgent2D and a NavigationAgent3D."
	)
	if nav_2d != null:
		return nav_2d
	return nav_3d


## Creates the action, optionally with a fixed target.
func _init(p_target_location: GdPAILocationData = null) -> void:
	target_location = p_target_location


## Returns a fresh instance so each plan step gets its own target.
func clone_for_plan() -> Action:
	return GoToAction.new()


## Takes the planner's [code]at_target[/code] binding as [member target_location].
func inject_binding(fact_name: String, value: Array) -> void:
	if fact_name == "at_target" and value.size() > 0:
		var obj = value[0]
		if obj is GdPAILocationData:
			target_location = obj


# Override
## Returns the straight-line distance to the target / 100, or [code]1.0[/code] while unbound.
func get_action_cost(
	agent_blackboard: GdPAIBlackboard,
	world_state: GdPAIBlackboard,
) -> float:
	var agent_location: SimObjectProxy = agent_blackboard.get_proxy_in_group("GdPAILocationData")

	# During planning, the planner injects the binding into the blackboard.
	# We should prefer this over any previously stored target_location.
	var actual_target = agent_blackboard.get_property("at_target")

	# Fallback to stored target_location if no binding was found in blackboard.
	if actual_target == null:
		actual_target = target_location

	if actual_target == null:
		# During planning before any binding is considered, return a reasonable heuristic cost
		# This allows the planner to consider GoToAction as a candidate
		return 1.0

	var sim_location: SimObjectProxy = world_state.get_object_for(actual_target)
	if sim_location == null:
		# If it's an ID, try to resolve it from the world state anyway.
		# GdPAIBlackboard.get_object_for now handles IDs.
		return 1.0

	if agent_location == null:
		return 1.0

	# Euclidean distance heuristic, normalized to be comparable with interaction costs
	var dist: float = (
		(agent_location.get_property("position") - sim_location.get_property("position")).length()
	)
	return dist / 100.0


# Override
## Requires the agent's entity and location, and a valid fixed target if one is set.
func get_validity_checks() -> Array[Precondition]:
	var checks: Array[Precondition] = []
	checks.append(Precondition.agent_has_property("entity"))
	checks.append(Precondition.agent_has_object_data_of_group("GdPAILocationData"))
	# Only check target_location validity if it's set (during execution, not planning)
	if target_location != null:
		checks.append(Precondition.check_is_object_valid(target_location))
	return checks


# Override
## Provides the wildcard fact [code]at_target[/code].
func get_provisions() -> Array[ProvisionSpec]:
	# Provide wildcard at_target fact that can satisfy any at_target requirement
	return [ProvisionSpec.fact_wildcard("at_target")]


# Override
func get_requirements() -> Array[RequirementSpec]:
	return []


# Override
## Moves the agent's simulated location to the target.
func simulate_effect(
	agent_blackboard: GdPAIBlackboard,
	world_state: GdPAIBlackboard,
) -> void:
	# Prefer blackboard binding during planning.
	var actual_target = agent_blackboard.get_property("at_target")

	if actual_target == null:
		actual_target = target_location

	if actual_target == null:
		return

	var sim_location: SimObjectProxy = world_state.get_object_for(actual_target)
	if sim_location != null:
		var agent_location: SimObjectProxy = agent_blackboard.get_proxy_in_group(
			"GdPAILocationData"
		)
		if agent_location != null:
			agent_location.set_property("position", sim_location.get_property("position"))


# Override
## Native form of [method simulate_effect]: copies the target's position onto the agent's
## location proxy, always through the [code]at_target[/code] binding as in GDScript. No
## ported scene pins a fixed target (only tests construct GoToAction with one), so no
## pinned-uid form is emitted.
func get_effect_spec() -> Array:
	return [
		{
			"op": "copy",
			"from": {"binding": "at_target", "prop": "position"},
			"to": {"agent_group": "GdPAILocationData", "prop": "position"},
		},
	]


# Override
## Fails without a valid target or navigation agent; caches both and resets arrival tracking.
func pre_perform_action(agent: GdPAIAgent) -> Action.Status:
	if target_location == null or not is_instance_valid(target_location):
		return Action.Status.FAILURE

	var entity: Node = agent.entity
	var nav_agent: Node = find_nav_agent(entity)
	if nav_agent == null:
		return Action.Status.FAILURE

	var dist_check: float = (
		ARRIVAL_THRESHOLD_2D if nav_agent is NavigationAgent2D else ARRIVAL_THRESHOLD_3D
	)

	# Cache the location data.
	var agent_location_data: GdPAILocationData = agent.blackboard.get_node_in_group(
		"GdPAILocationData"
	)

	# Set up state for navigation.
	set_state(agent, "nav_agent", nav_agent)
	set_state(agent, "dist_check", dist_check)
	set_state(agent, "agent_location", agent_location_data)
	set_state(agent, "target_set", false)
	set_state(agent, "target_reached", false)
	set_state(agent, "time_elapsed", 0.0)
	set_state(agent, "prior_positions", [])
	set_state(agent, "target_orig_position", target_location.position)

	return Action.Status.SUCCESS


# Override
## Navigates and applies the arrival rules from the class doc.
func perform_action(agent: GdPAIAgent, delta: float) -> Action.Status:
	if target_location == null or not is_instance_valid(target_location):
		return Action.Status.FAILURE

	var nav_agent: Node = get_state(agent, "nav_agent")
	var agent_location_data: GdPAILocationData = get_state(agent, "agent_location")
	var dist_check: float = get_state(agent, "dist_check")
	var target_orig_position = get_state(agent, "target_orig_position")

	# Keep track of how long we've been actively pursuing this target.
	var time_elapsed: float = get_state(agent, "time_elapsed")
	time_elapsed += delta
	set_state(agent, "time_elapsed", time_elapsed)

	# Keep [time, position] samples covering the last STUCK_WINDOW seconds.
	var prior_positions: Array = get_state(agent, "prior_positions")
	prior_positions.append([time_elapsed, agent_location_data.position])
	while prior_positions.size() > 1 and time_elapsed - prior_positions[1][0] >= STUCK_WINDOW:
		prior_positions.pop_front()
	set_state(agent, "prior_positions", prior_positions)

	# Begin walking to the target on the first action frame.
	if not get_state(agent, "target_set"):
		nav_agent.target_position = target_location.position
		set_state(agent, "target_set", true)

	# Terminating conditions.
	if nav_agent.is_navigation_finished():
		return _finish(agent, nav_agent, agent_location_data, Action.Status.SUCCESS)

	var oldest: Array = prior_positions[0]
	var stuck: bool = (
		time_elapsed - oldest[0] >= STUCK_WINDOW
		and (agent_location_data.position - oldest[1]).length() < dist_check
	)
	if stuck:
		# Blocked (by another agent, or an obstacle the navmesh does not carve). Only count it
		# as arrival when the agent is about as close as its path allows.
		var to_end: float = agent_location_data.position.distance_to(nav_agent.get_final_position())
		if to_end <= nav_agent.target_desired_distance * STUCK_ARRIVAL_FACTOR:
			return _finish(agent, nav_agent, agent_location_data, Action.Status.SUCCESS)
		return _finish(agent, nav_agent, agent_location_data, Action.Status.FAILURE)

	# Continue navigating.
	return Action.Status.RUNNING


## Stops navigation and returns [param status].
func _finish(
	agent: GdPAIAgent,
	nav_agent: Node,
	agent_location_data: GdPAILocationData,
	status: Action.Status,
) -> Action.Status:
	# post_perform_action only runs when the whole plan ends, so stop walking now.
	nav_agent.target_position = agent_location_data.position
	set_state(agent, "target_reached", status == Action.Status.SUCCESS)
	return status


# Override
## Resets the navigation target and erases this action's state, even without a valid target.
func post_perform_action(agent: GdPAIAgent) -> Action.Status:
	var nav_agent: Node = get_state(agent, "nav_agent")
	var agent_location_data: GdPAILocationData = get_state(agent, "agent_location")

	# Clear the navigation target.
	if (
		nav_agent != null
		and is_instance_valid(nav_agent)
		and agent_location_data != null
		and is_instance_valid(agent_location_data)
	):
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

	if target_location == null or not is_instance_valid(target_location):
		return Action.Status.FAILURE
	return Action.Status.SUCCESS


# Override
func get_title() -> String:
	return "Go To"


# Override
func get_description() -> String:
	return "Navigate to a target location."
