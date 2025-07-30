## Spatial actions are related to a physical object and contingent on proximity.  Moving the agent
## to the object is bundled into the action.  This class of actions uses Godot's navigation
## to test proximity, and relies on the GdPAI agent having a child NavigationAgent(2D/3D).
##[br]
##[br]
## NOTE: The agent still needs to move itself; this action just updates the navigation target of
## the agent's NavigationAgent.
class_name SpatialAction
extends Action

## Reference to the GdPAI location data that this action is tied to.  This is set when the action
## is created.
var object_location: GdPAILocationData
## Reference to the GdPAI interactable attributes.  This is set when the action is created.
var interactable_attribs: GdPAIInteractable

var has_done_pre: bool
var has_done_post: bool


# Override
func _init(object_location: GdPAILocationData, interactable_attribs: GdPAIInteractable):
	super()
	self.object_location = object_location
	self.interactable_attribs = interactable_attribs


# Override
func get_action_cost(agent_blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard) -> float:
	var agent_location: GdPAILocationData = agent_blackboard.get_first_object_in_group(
		"GdPAILocationData"
	)
	if not is_instance_valid(object_location):
		return INF
	var sim_location = world_state.get_object_by_uid(object_location.uid)
	if not is_instance_valid(sim_location):
		return INF
	# NOTE: This is a heuristic using Euclidean distance but not taking navigation obstacles into
	# 		account.  Using the navigation agent would be more expensive but yield a more accurate
	# 		cost.
	# TODO: Make navigation agent-based cost an option.  Maybe could configure in plugin.cfg?
	# 		Alternative would be to parameterize within the agent, but that could be tricky.
	var dist: float = (agent_location.position - sim_location.position).length()
	return dist


# Override
func get_validity_checks() -> Array[Precondition]:
	var checks: Array[Precondition] = []
	checks.append(Precondition.agent_has_property("entity"))
	checks.append(Precondition.agent_has_object_data_of_group("GdPAILocationData"))
	checks.append(Precondition.check_is_object_valid(object_location))
	checks.append(Precondition.check_is_object_valid(interactable_attribs))

	var can_get_to_check: Precondition = Precondition.new()
	can_get_to_check.eval_func = func(blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):
		# The target should be reachable by the agent.
		var entity: Node = blackboard.get_property("entity")
		var agent_location_data: GdPAILocationData = blackboard.get_first_object_in_group(
			"GdPAILocationData"
		)

		# nav_agent could be NavigationAgent2D or NavigationAgent3D depending on the setup.
		var nav_agent: Node
		var nav_agent_2d: Node = GdPAIUTILS.get_child_of_type(entity, NavigationAgent2D)
		var nav_agent_3d: Node = GdPAIUTILS.get_child_of_type(entity, NavigationAgent3D)
		# These shouldn't both be assigned.
		assert(nav_agent_2d == null or nav_agent_3d == null)
		if nav_agent_2d != null:
			nav_agent = nav_agent_2d
		elif nav_agent_3d != null:
			nav_agent = nav_agent_3d

		if nav_agent == null:
			# It is possible for objects and agents to be in 2D/3D space separately (I guess?).
			# But in those cases this particular action is not valid.
			return false

		# Optionally setting the interaction distance <= 0 bypasses the can_get_to constraint.
		if interactable_attribs.max_interaction_distance <= 0:
			return true

		# Override the entity's nav agent to test if it is possible to get to this object.
		var old_target_position = nav_agent.target_position
		nav_agent.target_position = object_location.position
		nav_agent.get_next_path_position()  # Compute the path.
		var final_dist: float = (object_location.position - nav_agent.get_final_position()).length()
		# Restore the nav agent's earlier state.
		nav_agent.target_position = old_target_position
		nav_agent.get_next_path_position()
		return final_dist < interactable_attribs.max_interaction_distance
	checks.append(can_get_to_check)

	return checks


# Override
func simulate_effect(agent_blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):
	# Simulate by teleporting the agent to the object's location.
	var agent_location: GdPAILocationData = agent_blackboard.get_first_object_in_group(
		"GdPAILocationData"
	)
	var sim_location = world_state.get_object_by_uid(object_location.uid)
	agent_location.position = sim_location.position


# Override
func pre_perform_action(agent: GdPAIAgent) -> Action.Status:
	# Failure state in the case the target has been freed since planning.
	if not is_instance_valid(object_location) or not is_instance_valid(interactable_attribs):
		return Action.Status.FAILURE

	var entity: Node = agent.blackboard.get_property("entity")

	# Cache the location data.
	var agent_location_data: GdPAILocationData = agent.blackboard.get_first_object_in_group(
		"GdPAILocationData"
	)
	agent.blackboard.set_property(uid_property("agent_location"), agent_location_data)

	# nav_agent could be NavigationAgent2D or NavigationAgent3D depending on the setup.
	var nav_agent: Node
	var nav_agent_2d: Node = GdPAIUTILS.get_child_of_type(entity, NavigationAgent2D)
	var nav_agent_3d: Node = GdPAIUTILS.get_child_of_type(entity, NavigationAgent3D)
	# These shouldn't both be assigned.
	assert(nav_agent_2d == null or nav_agent_3d == null)
	if nav_agent_2d != null:
		nav_agent = nav_agent_2d
	elif nav_agent_3d != null:
		nav_agent = nav_agent_3d
	assert(nav_agent != null)
	agent.blackboard.set_property(uid_property("nav_agent"), nav_agent)

	# Set up some flags for movement.
	agent.blackboard.set_property(uid_property("time_elapsed"), 0)
	agent.blackboard.set_property(uid_property("target_set"), false)
	agent.blackboard.set_property(uid_property("target_reached"), false)
	agent.blackboard.set_property(uid_property("prior_positions"), [agent_location_data.position])

	has_done_pre = true
	return Action.Status.SUCCESS


# Override
func perform_action(agent: GdPAIAgent, delta: float) -> Action.Status:
	# Failure state in the case the target has been freed.
	if not is_instance_valid(object_location) or not is_instance_valid(interactable_attribs):
		return Action.Status.FAILURE

	var nav_agent: Node = agent.blackboard.get_property(uid_property("nav_agent"))
	var agent_location_data: GdPAILocationData = agent.blackboard.get_property(
		uid_property("agent_location")
	)

	# Maintain a list of prior positions to check if the agent isn't moving.
	var prior_positions: Array = agent.blackboard.get_property(uid_property("prior_positions"))
	prior_positions.append(agent_location_data.position)
	if prior_positions.size() > 60:
		prior_positions.pop_front()
	agent.blackboard.set_property(uid_property("prior_positions"), prior_positions)

	# Keep track of how long we've been actively pursuing this object.
	var time_elapsed: float = agent.blackboard.get_property(uid_property("time_elapsed"))
	time_elapsed += delta
	agent.blackboard.set_property(uid_property("time_elapsed"), time_elapsed)

	# Begin walking to the target on the first action frame.
	if not agent.blackboard.get_property(uid_property("target_set")):
		nav_agent.target_position = object_location.position
		agent.blackboard.set_property(uid_property("target_set"), true)

	# Terminating conditions.
	# Either the navigation agent passes, or the agent has stopped for some other reason.
	var dist_traveled: float = (
		(prior_positions[-1] - prior_positions[0]).length() * delta * prior_positions.size()
	)
	# TODO: Parameterize the dist_traveled condition.  It could need different effective values in
	# 		2D or 3D, and for really slow agents.
	if nav_agent.is_navigation_finished() or (prior_positions.size() == 60 and dist_traveled < 1):
		# Pass if we have no interaction distance constraint.
		if interactable_attribs.max_interaction_distance <= 0:
			agent.blackboard.set_property(uid_property("target_reached"), true)
			return Action.Status.SUCCESS
		# Else, figure out the final distance and see if valid.
		var final_dist: float = (object_location.position - nav_agent.get_final_position()).length()
		if final_dist < interactable_attribs.max_interaction_distance:
			agent.blackboard.set_property(uid_property("target_reached"), true)
			return Action.Status.SUCCESS
		else:
			return Action.Status.FAILURE

	# Continue navigating.
	return Action.Status.RUNNING


# Override
func post_perform_action(agent: GdPAIAgent) -> Action.Status:
	# Corresponding failure state to what could skip pre actions.
	if not is_instance_valid(object_location) or not is_instance_valid(interactable_attribs):
		return Action.Status.FAILURE

	var nav_agent: Node = agent.blackboard.get_property(uid_property("nav_agent"))
	var agent_location_data: GdPAILocationData = agent.blackboard.get_property(
		uid_property("agent_location")
	)
	# Clear the navigation target.
	nav_agent.target_position = agent_location_data.position

	agent.blackboard.erase_property(uid_property("nav_agent"))
	agent.blackboard.erase_property(uid_property("agent_location"))
	agent.blackboard.erase_property(uid_property("target_set"))
	agent.blackboard.erase_property(uid_property("target_reached"))
	agent.blackboard.erase_property(uid_property("time_elapsed"))
	agent.blackboard.erase_property(uid_property("prior_positions"))

	has_done_post = true
	return Action.Status.SUCCESS
