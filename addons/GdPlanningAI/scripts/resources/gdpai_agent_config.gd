class_name GdPAIAgentConfig
extends Resource
## Serializable configuration resource for GdPAI agents.

## Planning strategy determines when and how often the agent replans.
enum PlanningStrategy {
	CONTINUOUS, ## Plan every frame (current behavior).
	ON_INTERVAL, ## Plan at fixed time intervals.
	ON_DEMAND, ## Plan only when explicitly requested.
}

## How the agent should approach planning.
@export var planning_strategy: PlanningStrategy = PlanningStrategy.CONTINUOUS
## @experimental: To support multithreading, your simulation must not directly change the scene
## tree!
## Whether this agent should do planning on a separate thread.
@export var use_multithreading: bool = false
## Thread priority when using multithreading.
@export var thread_priority: Thread.Priority = Thread.PRIORITY_LOW
## Maximum recursion depth for planning.
@export var max_recursion: int = 4
## Planning interval in seconds (only used for ON_INTERVAL strategy).
@export var planning_interval: float = 0.5
## Blackboard plan for the agent.
@export var blackboard_plan: GdPAIBlackboardPlan = GdPAIBlackboardPlan.new()
