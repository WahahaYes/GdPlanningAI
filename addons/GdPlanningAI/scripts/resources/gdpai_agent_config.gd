class_name GdPAIAgentConfig
extends Resource
## Serializable configuration resource for GdPAI agents.

## Planning strategy determines when and how often the agent replans.
enum PlanningStrategy {
	CONTINUOUS, ## Plan every frame (current behavior).
	ON_INTERVAL, ## Plan at fixed time intervals.
	ON_DEMAND, ## Plan only when explicitly requested.
	ON_INTERVAL_FORCED, ## Force planning at intervals, even if plan is active.
}

## How the agent should approach planning.
@export var planning_strategy: PlanningStrategy = PlanningStrategy.CONTINUOUS
## Planning interval in seconds (only used for ON_INTERVAL strategy).
@export var planning_interval: float = 0.5
## Maximum planning search depth. Branches deeper than this are pruned.
## Minimum enforced by the scheduler is 1.
@export var max_recursion: int = 100
## Maximum iterations per planning step before yielding to the main thread.
## Minimum enforced by the scheduler is 100.
@export var iteration_budget: int = 20000
## Blackboard plan for the agent.
@export var blackboard_plan: GdPAIBlackboardPlan = GdPAIBlackboardPlan.new()
## Behavior configurations that provide goals, actions, and property updaters.
@export var behavior_configs: Array[GdPAIBehaviorConfig] = []
