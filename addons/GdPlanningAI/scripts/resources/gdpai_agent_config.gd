class_name GdPAIAgentConfig
extends Resource
## Per-agent settings: when it plans, how much search effort it may spend, its starting
## blackboard and its behaviors. Assign to [member GdPAIAgent.config].

enum PlanningStrategy {
	CONTINUOUS,  ## Plan whenever there is no plan.
	ON_INTERVAL,  ## Every [member planning_interval] seconds, if there is no plan.
	ON_DEMAND,  ## Only on [method GdPAIAgent.manually_start_plan].
	ON_INTERVAL_FORCED,  ## Every [member planning_interval] seconds, replacing any plan.
}

## When the agent plans.
@export var planning_strategy: PlanningStrategy = PlanningStrategy.CONTINUOUS
## Seconds between planning checks for the interval strategies. Each agent starts at a random
## offset.
@export var planning_interval: float = 0.5
## Seconds to wait after a failed plan before planning again on its own; [code]0[/code] retries
## at once. Idle (empty) plans do not count as failures, and
## [method GdPAIAgent.manually_start_plan] ignores the wait.
@export var failure_backoff: float = 0.0
## Maximum search depth; deeper branches are pruned.
@export var max_recursion: int = 100
## Search iterations per step before yielding to a later frame.
@export var iteration_budget: int = 20000
## Milliseconds per planning step before yielding; [code]0[/code] disables time-slicing.
@export var time_slice_ms: int = 10
## Starting contents of the agent's blackboard.
@export var blackboard_plan: GdPAIBlackboardPlan = GdPAIBlackboardPlan.new()
## Behaviors supplying the agent's goals, actions and updaters.
@export var behavior_configs: Array[GdPAIBehaviorConfig] = []
