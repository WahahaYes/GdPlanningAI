extends GutTest
## Switching planning strategy retires the interval timer cleanly.

const AGENT_CONFIG: GdPAIAgentConfig = preload(
	"res://examples/configs/hunger_wander_agent_config.tres"
)


## A timeout from the replaced timer, delivered after the switch, is ignored.
func test_stale_timer_timeout_after_switch_is_ignored() -> void:
	var agent: GdPAIAgent = GdPAIAgent.new()
	agent.config = AGENT_CONFIG
	add_child_autofree(agent)
	agent.set_planning_strategy(GdPAIAgentConfig.PlanningStrategy.ON_INTERVAL, 1.0)
	var old_timer: Timer = agent._planning_timer
	assert_not_null(old_timer, "Interval planning should own a timer")

	agent.set_planning_strategy(GdPAIAgentConfig.PlanningStrategy.ON_DEMAND)
	assert_true(old_timer.is_stopped(), "The replaced timer should be stopped")
	old_timer.timeout.emit()
	assert_null(agent._planning_timer, "On-demand planning keeps no timer")
