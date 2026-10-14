class_name GoToAction
extends Action
## Generic navigation action: walks the agent's entity to a [GdPAILocationData] using its
## [NavigationAgent2D] or [NavigationAgent3D].[br]
##[br]
## Add one [GoToAction] to an agent's actions (usually from a [GdPAIBehaviorConfig]). It provides
## the wildcard fact [code]at_target[/code], so any action that declares
## [code]RequirementSpec.fact("at_target", [location_data])[/code] makes the planner insert a
## navigation step before it; the planner binds the concrete location, and the agent hands it to
## [method inject_binding] before execution. Extend it, or write your own action providing
## [code]ProvisionSpec.fact_wildcard("at_target")[/code], for other movement systems.[br]
##[br]
## [b]Requirements on the entity:[/b] the agent's entity must contain exactly one navigation agent
## (2D or 3D, found recursively below the entity) and the agent must own a [GdPAILocationData]
## (it is the position the action reads).[br]
##[br]
## [b]Arrival rules[/b] (checked every frame in [method perform_action], in this order):[br]
## 1. If the navigation agent reports [code]is_navigation_finished()[/code], the action succeeds.[br]
## 2. Otherwise the agent counts as [i]stuck[/i] when, over the last [constant STUCK_WINDOW]
## seconds of this action, it moved less than [constant ARRIVAL_THRESHOLD_2D] (2D) or
## [constant ARRIVAL_THRESHOLD_3D] (3D). A stuck agent whose distance to the navigation agent's
## final path position is at most [code]target_desired_distance[/code] *
## [constant STUCK_ARRIVAL_FACTOR] has arrived as close as it can get and the action succeeds;
## a stuck agent further away fails, so the plan is aborted and the agent replans.[br]
## 3. Otherwise the action keeps running.[br]
##[br]
## When the action finishes (either result), it immediately stops navigation by setting the
## navigation target to the agent's current position, because [method post_perform_action] only
## runs when the whole plan ends. It also fails if the target is freed or the entity has no
## navigation agent.

## Minimum distance (pixels) the agent must move over [constant STUCK_WINDOW] in 2D to not count as
## stuck.
const ARRIVAL_THRESHOLD_2D: float = 8.0
## Minimum distance (meters) the agent must move over [constant STUCK_WINDOW] in 3D to not count
## as stuck.
const ARRIVAL_THRESHOLD_3D: float = 0.1
## Length in seconds of the sliding window used to detect that the agent is stuck.
const STUCK_WINDOW: float = 1.0
## A stuck agent within this many multiples of the navigation agent's
## [code]target_desired_distance[/code] of its path's end has arrived as close as it can get;
## further away, the action fails so the agent replans.
const STUCK_ARRIVAL_FACTOR: float = 2.0


## Returns the [NavigationAgent2D] or [NavigationAgent3D] child of [param entity],
## or [code]null[/code] if neither is present. The search covers [param entity] and all of its
## descendants. Asserts if both kinds are present.
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


## The location to navigate to. Normally filled in by [method inject_binding] from the planner's
## [code]at_target[/code] binding; set it through [code]_init[/code] to give a fixed target.
## Execution fails if it is [code]null[/code] or freed.
var target_location: GdPAILocationData


## Creates the action, optionally with a fixed [param p_target_location].
func _init(p_target_location: GdPAILocationData = null) -> void:
	target_location = p_target_location


## Returns a fresh [GoToAction] with [member target_location] cleared, so each occurrence in a plan
## gets its own target. The agent re-injects the bindings through [method inject_binding]
## before executing.
func clone_for_plan() -> Action:
	return GoToAction.new()


## Receives a planner binding for this action's wildcard provision.[br]
##[br]
## Called by the agent on the main thread before each execution hook, with the objects the planner
## bound to [param fact_name] in [param value]. Only [code]at_target[/code] is handled: the first
## value, if it is a [GdPAILocationData], becomes [member target_location]. Other names and types
## are ignored. Custom actions with wildcard provisions can define the same method.
func inject_binding(fact_name: String, value: Array) -> void:
	if fact_name == "at_target" and value.size() > 0:
		var obj = value[0]
		if obj is GdPAILocationData:
			target_location = obj


# Override
## Estimates the cost of walking: the straight-line distance between the agent's simulated
## location and the target, divided by 100 so it is comparable with interaction costs. Returns
## [code]1.0[/code] as a neutral guess while the planner has not bound a target yet or the
## locations cannot be found in the simulation. Planning-time hook, see
## [method Action.get_action_cost] for its constraints.
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
## Requires the agent to have an [code]entity[/code] property and a [GdPAILocationData] object,
## and, when [member target_location] is set, that the target is still valid. It is [code]null[/code]
## on the instance the planner sees, so that check only applies to fixed-target instances.
func get_validity_checks() -> Array[Precondition]:
	var checks: Array[Precondition] = []
	checks.append(Precondition.agent_has_property("entity"))
	checks.append(Precondition.agent_has_object_data_of_group("GdPAILocationData"))
	# Only check target_location validity if it's set (during execution, not planning)
	if target_location != null:
		checks.append(Precondition.check_is_object_valid(target_location))
	return checks


# Override
## Provides the wildcard fact [code]at_target[/code], satisfying any requirement
## [code]RequirementSpec.fact("at_target", [...])[/code].
func get_provisions() -> Array[ProvisionSpec]:
	# Provide wildcard at_target fact that can satisfy any at_target requirement
	return [ProvisionSpec.fact_wildcard("at_target")]


# Override
## Navigation needs nothing from earlier actions, so this returns an empty array.
func get_requirements() -> Array[RequirementSpec]:
	return []


# Override
## Moves the agent's simulated [GdPAILocationData] to the target's simulated position, so later
## steps are planned as if the agent had arrived. Does nothing while no target is bound.
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
## Fails if the target is missing or freed, or the entity has no navigation agent. Otherwise caches
## the navigation agent and the agent's location data and resets the arrival tracking state.
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
## Starts navigation on the first frame, then applies the arrival rules described in the class
## documentation each frame. Returns [constant Action.Status.RUNNING] until the agent arrives,
## succeeds on arrival, and fails when stuck away from the end of its path or when the target is
## freed.
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


## Stops navigation and records whether the target was reached. Returns [param status].
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
## Plan-end cleanup: resets the navigation target to the agent's position and erases this
## action's state. Fails early, without cleaning up, if the target is no longer valid.
func post_perform_action(agent: GdPAIAgent) -> Action.Status:
	if target_location == null or not is_instance_valid(target_location):
		return Action.Status.FAILURE

	var nav_agent: Node = get_state(agent, "nav_agent")
	var agent_location_data: GdPAILocationData = get_state(agent, "agent_location")

	# Clear the navigation target.
	if nav_agent != null and agent_location_data != null and is_instance_valid(agent_location_data):
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


# Override
## Returns [code]"Go To"[/code].
func get_title() -> String:
	return "Go To"


# Override
## Returns a short description of the navigation action.
func get_description() -> String:
	return "Navigate to a target location."
