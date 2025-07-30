## This is a base class to extend with actual AI actions.  During planning, actions are simulated
## instantaneously.  After planning, actions are carried out by the agent in real time.
class_name Action
extends RefCounted

## A reference to utils for the GdPAI addon.
const GdPAIUTILS: Resource = preload("res://addons/GdPlanningAI/utils.gd")
## Return states for actions during true simulation.
enum Status { FAILURE, RUNNING, SUCCESS }
## A uid is automatically allocated for actions so that they can put unique properties into the
## blackboards as needed without risk of collisions with other actions.
var uid: String
## Counter for allocating new uids.
static var _uid_counter: int = 0


# Override
func _init():
	uid = str(_uid_counter)
	_uid_counter += 1


## List of static preconditions needed for the action to be considered.  This is
## evaluated at the time of assigning wordly actions (so, there is no need to grab sim data).
##[br]
##[br]
## For multithreaded planning, it is possible to await information with GdPAIUTILS.await_callv(..).
func get_validity_checks() -> Array[Precondition]:
	return []


## Computes the cost to complete this action.  This is done at simulation time, so if
## referencing any object tied to the action make sure to first get the simulated version
## with world_state.get_object_by_uid(<object>.uid).
##[br]
##[br]
## Can assume that validity checks are true at this point, EXCEPT continued references existing in
## the scene tree.  If a reference is invalid, returning INF tells the planner to skip this action.
## For multithreaded planning, it is possible to await information with GdPAIUTILS.await_callv(..).
func get_action_cost(agent_blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard) -> float:
	return 0


## Lists the preconditions necessary for this action to be carried out.  This is evaluated
## during the simulation, so if referencing an object tied to the action make sure to first
## get the simulated version with world_state.get_object_by_uid(<object>.uid).
##[br]
##[br]
## Can assume that validity checks are true at this point, EXCEPT continued references existing in
## the scene tree.  If a reference is invalid, returning INF tells the planner to skip this action.
## For multithreaded planning, it is possible to await information with GdPAIUTILS.await_callv(..).
func get_preconditions() -> Array[Precondition]:
	return []


## Simulates onto the agent's blackboards the change that would occur if this action is taken out.
## Does not return a value; this modifies the blackboards in-place.  If referencing an object
## make sure to first get the simulated version with world_state.get_object_by_uid(<object>.uid).
## This method SHOULD NOT make any changes inside the scene tree.
##[br]
##[br]
## Can assume that any validity checks are already true.
func simulate_effect(agent_blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):
	pass


## In case any attribute is reliant on an earlier step (like knowing WHAT an agent picks up, for
## example), this function is called on earlier traversals.  This can be useful because the
## action plan is being decided in reverse order, so at times it is impossible to know exactly
## what is going to occur from a later action until we simulate earlier actions.  A prime example
## being <pickup> -> <eat>.  When <eat> is simulated, the agent isn't holding anything so we don't
## know what would've been eaten.  But after <pickup> is simulated, we can refer back to the agent
## to figure out the object then determine how many hunger points that food is going to restore.
func reverse_simulate_effect(agent_blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):
	pass


## Perform any pre computations for the action.  All actions' pre_perform methods are called at
## the start of an action chain, regardless of if the plan succeeds or not.  Can return
## Status.FAILURE to indicate the plan should be aborted.
##[br]
##[br]
## At this point, validity checks true during planning could be false in the real world.
func pre_perform_action(agent: GdPAIAgent) -> Status:
	return Status.SUCCESS


## Perform the action in the actual simulation.  Can return RUNNING for actions that have a
## duration.
##[br]
##[br]
## Need to monitor any validity checks that could become false after some time.
func perform_action(agent: GdPAIAgent, delta: float) -> Status:
	return Status.SUCCESS


## Perform any post computations for the action.  Status currently doesn't matter because the plan
## is already done.  All actions' post_perform methods are called, regardless of if the plan
## succeeded.  These methods should make sure to safely de-allocate anything created for the action.
func post_perform_action(agent: GdPAIAgent) -> Status:
	return Status.SUCCESS


## Generates a String that appends this action's uid to allow for easier blackboard referencing.
func uid_property(prop: String) -> String:
	return "%s_%s" % [uid, prop]
