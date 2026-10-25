extends GutTest
## [signal GdPAIAgent.plan_delivered] announces every installed plan, and the agent's accessors
## expose the action at the current step and whether a plan request is in flight.
## Plans are installed through [code]_on_plan_ready[/code], as the scheduler does.

const FAILED: Dictionary = {"success": false}

## Results handed to the listener, oldest first.
var _delivered: Array[Dictionary] = []


func after_each() -> void:
	_delivered.clear()
	_cancel_jobs()


## Collects a delivered plan result.
func _on_plan_delivered(result: Dictionary) -> void:
	_delivered.append(result)


## Builds a goal-holding agent with an entity and a world node, so a request really is in flight;
## frames do not drive it.
func _make_agent() -> GdPAIAgent:
	var world_node: GdPAIWorldNode = GdPAIWorldNode.new()
	world_node.blackboard_plan = GdPAIBlackboardPlan.new()
	add_child_autofree(world_node)

	var agent: GdPAIAgent = GdPAIAgent.new()
	agent.config = GdPAIAgentConfig.new()
	agent.config.planning_strategy = GdPAIAgentConfig.PlanningStrategy.ON_DEMAND
	agent.entity = add_child_autofree(Node.new())
	add_child_autofree(agent)
	agent.set_process(false)
	agent.world_node = world_node
	agent.goals.append(Goal.new())
	agent.plan_delivered.connect(_on_plan_delivered)
	return agent


## Returns [param count] distinct actions for the agent to deserialize a chain from.
func _make_actions(count: int) -> Array[Action]:
	var actions: Array[Action] = []
	for i in range(count):
		actions.append(Action.new())
	return actions


## Delivers [param result] as the answer to the in-flight request.
func _answer(agent: GdPAIAgent, result: Dictionary) -> void:
	agent._inflight_generation = agent._plan_generation
	agent._on_plan_ready(result)


## Stops any real planning job a test started, so it cannot deliver after the agent is freed.
func _cancel_jobs() -> void:
	var scheduler: GdPAIPlanScheduler = GdPAIAutoload.get_scheduler()
	if scheduler != null:
		scheduler.cancel_all_jobs()


## A successful plan is delivered once, after its chain is installed.
func test_successful_plan_is_delivered() -> void:
	var agent: GdPAIAgent = _make_agent()
	agent._last_submitted_actions = _make_actions(2)
	var result: Dictionary = {
		"success": true,
		"total_cost": 3.0,
		"goal_index": 0,
		"action_chain": [0, 1],
		"action_bindings": [],
	}
	_answer(agent, result)

	assert_eq(_delivered.size(), 1, "One delivery per installed plan")
	assert_true(_delivered[0]["success"], "The result reports the success")
	assert_eq(_delivered[0]["action_chain"], [0, 1], "The planner chain travels unchanged")
	assert_eq(agent.get_current_plan().size(), 2, "The chain is installed before the signal")
	assert_eq(agent.get_current_plan_step(), -1, "The installed plan has not started")


## A failed plan is delivered too, with no action left to run.
func test_failed_plan_is_delivered() -> void:
	var agent: GdPAIAgent = _make_agent()
	agent._last_submitted_actions = _make_actions(1)
	_answer(agent, FAILED)

	assert_eq(_delivered.size(), 1, "A failure is delivered")
	assert_false(_delivered[0]["success"], "The result reports the failure")
	assert_null(agent.get_current_action(), "A failed plan has no current action")
	assert_true(agent.get_current_plan().is_empty(), "A failed plan installs no chain")


## A stale result is discarded, not installed, so it is not delivered.
func test_stale_result_is_not_delivered() -> void:
	var agent: GdPAIAgent = _make_agent()
	agent._plan_generation = 2
	agent._inflight_generation = 1
	agent._on_plan_ready(FAILED)

	assert_eq(_delivered.size(), 0, "A discarded result is not installed")
	assert_true(agent.get_current_plan().is_empty(), "A discarded result installs no chain")


## The current action is null before the plan starts and after it ends, and the step's action
## in between.
func test_current_action_follows_the_step() -> void:
	var agent: GdPAIAgent = _make_agent()
	var actions: Array[Action] = _make_actions(2)
	agent._current_action_chain = actions

	agent._current_plan_step = -1
	assert_null(agent.get_current_action(), "No action before the pre-perform phase")
	agent._current_plan_step = 0
	assert_eq(agent.get_current_action(), actions[0], "First action at step 0")
	agent._current_plan_step = 1
	assert_eq(agent.get_current_action(), actions[1], "Second action at step 1")
	agent._current_plan_step = 2
	assert_null(agent.get_current_action(), "No action once the chain ran out")
	agent._current_plan_step = 3
	assert_null(agent.get_current_action(), "No action during the post-action cleanup")


## The waiting flag is true only between a request and its answer.
func test_waiting_flag_tracks_the_request() -> void:
	var agent: GdPAIAgent = _make_agent()
	assert_false(agent.is_waiting_for_plan(), "Idle before any request")

	agent.manually_start_plan()
	assert_true(agent.is_waiting_for_plan(), "A submitted request is in flight")

	_cancel_jobs()
	_answer(agent, FAILED)
	assert_false(agent.is_waiting_for_plan(), "The answer ends the wait")
