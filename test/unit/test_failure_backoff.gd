extends GutTest
## [member GdPAIAgentConfig.failure_backoff] delays automatic planning after a failed plan.
## Submissions are counted by [code]_plan_generation[/code]; no world node is needed for that.

const FAILED: Dictionary = {"success": false}
const IDLE: Dictionary = {
	"success": true, "action_chain": [], "action_bindings": [], "goal_index": 0
}


## Builds a goal-holding agent with [param strategy] and a 1 s backoff; frames do not drive it.
func _make_agent(strategy: GdPAIAgentConfig.PlanningStrategy) -> GdPAIAgent:
	var agent: GdPAIAgent = GdPAIAgent.new()
	agent.config = GdPAIAgentConfig.new()
	agent.config.planning_strategy = strategy
	agent.config.failure_backoff = 1.0
	agent.entity = add_child_autofree(Node.new())
	add_child_autofree(agent)
	agent.set_process(false)
	agent.goals.append(Goal.new())
	return agent


## Delivers [param result] as the answer to the in-flight request.
func _answer(agent: GdPAIAgent, result: Dictionary) -> void:
	agent._inflight_generation = agent._plan_generation
	agent._on_plan_ready(result)


## CONTINUOUS waits out the backoff after a failure, then plans again.
func test_continuous_waits_after_failure() -> void:
	var agent: GdPAIAgent = _make_agent(GdPAIAgentConfig.PlanningStrategy.CONTINUOUS)
	_answer(agent, FAILED)
	var submitted: int = agent._plan_generation
	agent._process(0.6)
	assert_eq(agent._plan_generation, submitted, "No submission during the backoff")
	agent._process(0.6)
	assert_eq(agent._plan_generation, submitted + 1, "Plans again once the backoff elapsed")


## An idle (empty, successful) plan is not a failure.
func test_idle_plan_does_not_back_off() -> void:
	var agent: GdPAIAgent = _make_agent(GdPAIAgentConfig.PlanningStrategy.CONTINUOUS)
	_answer(agent, IDLE)
	var submitted: int = agent._plan_generation
	agent._process(0.1)
	assert_eq(agent._plan_generation, submitted + 1, "Replans on the next frame")


## Interval timeouts during the backoff are skipped.
func test_interval_skips_timeouts_during_backoff() -> void:
	var agent: GdPAIAgent = _make_agent(GdPAIAgentConfig.PlanningStrategy.ON_INTERVAL_FORCED)
	_answer(agent, FAILED)
	var submitted: int = agent._plan_generation
	agent._on_planning_timer_timeout()
	assert_eq(agent._plan_generation, submitted, "No submission during the backoff")
	agent._process(1.1)
	agent._on_planning_timer_timeout()
	assert_eq(agent._plan_generation, submitted + 1, "Plans on the first timeout after it")


## A successful plan ends the backoff early.
func test_success_clears_backoff() -> void:
	var agent: GdPAIAgent = _make_agent(GdPAIAgentConfig.PlanningStrategy.CONTINUOUS)
	_answer(agent, FAILED)
	agent.manually_start_plan()
	_answer(agent, IDLE)
	var submitted: int = agent._plan_generation
	agent._process(0.1)
	assert_eq(agent._plan_generation, submitted + 1, "No wait left after a success")


## A manual request ignores the backoff.
func test_manual_start_ignores_backoff() -> void:
	var agent: GdPAIAgent = _make_agent(GdPAIAgentConfig.PlanningStrategy.ON_DEMAND)
	_answer(agent, FAILED)
	var submitted: int = agent._plan_generation
	agent.manually_start_plan()
	assert_eq(agent._plan_generation, submitted + 1, "Manual planning is not delayed")
