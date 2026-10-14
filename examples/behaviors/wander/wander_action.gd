class_name WanderAction
extends GoToAction
## Navigates the agent to a random nearby point.
##[br]
##[br]
## Demonstrates a navigation action with no specific world target — the
## destination is computed at execution time from the agent's current position.
## Inherits from [GoToAction] to reuse navigation logic.[br]
##[br]
## Two things differ from a plain [GoToAction]. It has no requirements and provides
## nothing, so interaction actions can never use it to satisfy [code]at_target[/code]:
## only a real Go To toward a real object may do that. And its target is invented at
## execution time: a throwaway [Node2D] or [Node3D] is wrapped in a [GdPAILocationData]
## so the inherited navigation code can treat it like any other location, then freed
## in [method post_perform_action].[br]
##[br]
## During planning the step is simulated as a random move of [member wander_distance];
## the planner only needs to know the agent ends up far from where it started.

## How far from the agent's current position the wander target may be.
var wander_distance: float
## Temporary node for the wander target location.
var _wander_target_node: Node
## Temporary location data for the wander target.
var _wander_target_location: GdPAILocationData


func _init(p_wander_distance: float) -> void:
	super()
	wander_distance = p_wander_distance


## Returns a fresh [WanderAction] instance with the same [member wander_distance].
## Needed because the temporary target is stored on the instance: each plan gets its own
## copy so executions never share a target node.
func clone_for_plan() -> Action:
	return WanderAction.new(wander_distance)


# Override
func get_validity_checks() -> Array[Precondition]:
	# Wander doesn't need to check a specific target location; it only needs a body to
	# move and a location record to read its position from.
	var checks: Array[Precondition] = []
	checks.append(Precondition.agent_has_property("entity"))
	checks.append(Precondition.agent_has_object_data_of_group("GdPAILocationData"))
	return checks


# Override
func get_action_cost(
	_agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> float:
	# Wander cost is fixed since target is computed at runtime; a flat 1 keeps it
	# cheap but never free, so a real goal-satisfying plan is not preferred over it by accident.
	return 1.0


# Override
func get_preconditions() -> Array[Precondition]:
	return []


# Override
func get_provisions() -> Array[ProvisionSpec]:
	# WanderAction doesn't provide at_target - only the agent's GoToAction should. If it
	# did, the planner could use "wander" to satisfy an interaction's location requirement
	# and plan to pick up an item from wherever the random step lands.
	return []


# Override
func simulate_effect(
	agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> void:
	# Simulate moving wander_distance in a random direction. The origin is recorded the
	# first time so WanderGoal can measure how far the simulated agent has moved.
	var sim_location: SimObjectProxy = agent_blackboard.get_proxy_in_group("GdPAILocationData")
	var current_pos: Variant = sim_location.get_property("position")

	var random_dir: Vector2 = Vector2.from_angle(randf_range(0, TAU))

	if current_pos is Vector2:
		sim_location.set_property("position", current_pos + random_dir * wander_distance)
		# Also track origin position for wander goal evaluation
		if not sim_location.has_property("wander_origin"):
			sim_location.set_property("wander_origin", current_pos)
	elif current_pos is Vector3:
		var random_dir_3d: Vector3 = Vector3(random_dir.x, 0, random_dir.y)
		sim_location.set_property("position", current_pos + random_dir_3d * wander_distance)
		# Also track origin position for wander goal evaluation
		if not sim_location.has_property("wander_origin"):
			sim_location.set_property("wander_origin", current_pos)


# Override
func pre_perform_action(agent: GdPAIAgent) -> Action.Status:
	# Pick the real target now, at execution time, from the agent's actual position.
	var location_data: GdPAILocationData = agent.blackboard.get_node_in_group("GdPAILocationData")

	var random_dir: Vector2 = Vector2.from_angle(deg_to_rad(randf_range(-180, 180)))
	var target_pos: Variant

	if location_data.position is Vector2:
		target_pos = location_data.position + random_dir * wander_distance
	else:
		var random_dir_3d: Vector3 = Vector3(random_dir.x, 0, random_dir.y)
		target_pos = location_data.position + random_dir_3d * wander_distance

	# Create temporary node for the target location
	if target_pos is Vector2:
		_wander_target_node = Node2D.new()
		(_wander_target_node as Node2D).position = target_pos
	else:
		_wander_target_node = Node3D.new()
		(_wander_target_node as Node3D).position = target_pos

	# Add to scene tree so GdPAILocationData can reference it
	agent.entity.get_tree().root.add_child(_wander_target_node)

	# Create location data for the temporary node
	_wander_target_location = GdPAILocationData.new()
	if target_pos is Vector2:
		_wander_target_location.location_node_2d = _wander_target_node as Node2D
	else:
		_wander_target_location.location_node_3d = _wander_target_node as Node3D
	target_location = _wander_target_location

	# Call parent to handle navigation setup
	return super(agent)


# Override
func perform_action(agent: GdPAIAgent, delta: float) -> Action.Status:
	# Delegate navigation to parent GoToAction
	return super(agent, delta)


# Override
func post_perform_action(agent: GdPAIAgent) -> Action.Status:
	# Clean up temporary node and location data
	if _wander_target_node != null and is_instance_valid(_wander_target_node):
		_wander_target_node.queue_free()
		_wander_target_node = null
	_wander_target_location = null

	# Call parent to handle navigation cleanup
	return super(agent)


# Override
func get_title() -> String:
	return "Wander"


# Override
func get_description() -> String:
	return "Navigate to a random nearby point."
