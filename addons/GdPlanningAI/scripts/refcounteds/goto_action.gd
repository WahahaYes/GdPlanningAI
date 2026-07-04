class_name GoToAction
extends Action
## A generic navigation action provided by the agent.
## This action provides a wildcard `at_target` provision that can satisfy
## any interaction action requiring the agent to be at a specific location.

## Arrival distance threshold for 2D navigation (pixels).
const ARRIVAL_THRESHOLD_2D: float = 8.0
## Arrival distance threshold for 3D navigation (meters).
const ARRIVAL_THRESHOLD_3D: float = 0.1


## Returns the [NavigationAgent2D] or [NavigationAgent3D] child of [param entity],
## or [code]null[/code] if neither is present.
## Asserts if both are present simultaneously.
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


## The target location to navigate to. This will be bound during planning
## when the action is selected to satisfy an interaction's requirement.
var target_location: GdPAILocationData


# Override
func _init(p_target_location: GdPAILocationData = null) -> void:
	target_location = p_target_location


## Returns a fresh [GoToAction] instance with [member target_location] cleared.
## The planner will re-inject bindings at execution time.
func clone_for_plan() -> Action:
	return GoToAction.new()


## Injects a planner-provided binding into this action instance.
##[br]
##[br]
## Called by the bridge after planning to set concrete values for wildcard provisions.
## For [code]at_target[/code], sets the [member target_location] to the provided location data.
func inject_binding(fact_name: String, value: Array) -> void:
	if fact_name == "at_target" and value.size() > 0:
		var obj = value[0]
		if obj is GdPAILocationData:
			target_location = obj


# Override
func get_action_cost(
	agent_blackboard: GdPAIBlackboard,
	world_state: GdPAIBlackboard,
) -> float:
	var agent_location: SimObjectProxy = agent_blackboard.get_proxy_in_group("GdPAILocationData")

	# During planning, the planner injects the binding into the blackboard.
	# We should prefer this over any previously stored target_location.
	var actual_target = null
	var binding = agent_blackboard.get_property("at_target")

	if binding != null:
		if binding is Array:
			var flattened = binding
			while flattened.size() > 0 and flattened[0] is Array:
				flattened = flattened[0]
			if flattened.size() > 0:
				actual_target = flattened[0]
		else:
			actual_target = binding

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
func get_validity_checks() -> Array[Precondition]:
	var checks: Array[Precondition] = []
	checks.append(Precondition.agent_has_property("entity"))
	checks.append(Precondition.agent_has_object_data_of_group("GdPAILocationData"))
	# Only check target_location validity if it's set (during execution, not planning)
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
	world_state: GdPAIBlackboard,
) -> void:
	# Prefer blackboard binding during planning.
	var actual_target = null
	var binding = agent_blackboard.get_property("at_target")
	if binding != null:
		if binding is Array:
			var flattened = binding
			while flattened.size() > 0 and flattened[0] is Array:
				flattened = flattened[0]
			if flattened.size() > 0:
				actual_target = flattened[0]
		else:
			actual_target = binding

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
func get_title() -> String:
	return "Go To"


# Override
func get_description() -> String:
	return "Navigate to a target location."
