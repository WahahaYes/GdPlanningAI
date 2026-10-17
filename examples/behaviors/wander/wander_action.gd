class_name WanderAction
extends GoToAction
## Walks to a random nearby point. Reuses [GoToAction]'s navigation with a temporary target
## created at execution time and freed in [method post_perform_action].[br]
## It provides no [code]at_target[/code], so interactions can never be satisfied by wandering.

## Distance of each wander step.
var wander_distance: float
## Temporary node for the wander target location.
var _wander_target_node: Node
## Temporary location data for the wander target.
var _wander_target_location: GdPAILocationData


func _init(p_wander_distance: float) -> void:
	super()
	wander_distance = p_wander_distance


## Returns a fresh instance so plans never share a temporary target.
func clone_for_plan() -> Action:
	return WanderAction.new(wander_distance)


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
	return 1.0


# Override
func get_preconditions() -> Array[Precondition]:
	return []


# Override
func get_provisions() -> Array[ProvisionSpec]:
	return []


# Override
## Records the step without moving the agent. The direction is only drawn at execution, so a
## simulated position would mislead later costs and position checks.
func simulate_effect(
	agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> void:
	agent_blackboard.set_property("wandered", true)


# Override
func pre_perform_action(agent: GdPAIAgent) -> Action.Status:
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
