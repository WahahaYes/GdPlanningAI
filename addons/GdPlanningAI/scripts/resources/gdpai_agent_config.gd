class_name GdPAIAgentConfig
extends Resource
## Per-agent configuration resource: when and how an agent plans, how much search effort it may
## spend, its starting blackboard, and its behaviors (which supply goals, actions and property
## updaters). Assign it to [member GdPAIAgent.config]; it can be saved and shared between agents.

## Determines when the agent starts a new planning request.
enum PlanningStrategy {
	CONTINUOUS,  ## Plan whenever there is no active plan and no request in flight (checked every frame).
	ON_INTERVAL,  ## Check every [member planning_interval] seconds and plan if there is no active plan.
	ON_DEMAND,  ## Plan only when [method GdPAIAgent.manually_start_plan] is called.
	ON_INTERVAL_FORCED,  ## Plan every [member planning_interval] seconds, replacing a running plan.
}

## When the agent plans. See [enum PlanningStrategy].
@export var planning_strategy: PlanningStrategy = PlanningStrategy.CONTINUOUS
## Seconds between planning checks. Used by [constant PlanningStrategy.ON_INTERVAL] and
## [constant PlanningStrategy.ON_INTERVAL_FORCED] only. Each agent starts its timer with a random
## offset below this value so agents do not all plan on the same frame.
@export var planning_interval: float = 0.5
## Maximum search depth (open requirements in a branch). Deeper branches are pruned, which also
## stops runaway requirement chains. Values below 1 are raised to 1 by the scheduler.
@export var max_recursion: int = 100
## Search iterations the planner may run in one step before it yields and resumes on a later
## frame. Lower values spread long searches over more frames. Values below 100 are raised to 100.
@export var iteration_budget: int = 20000
## Wall-clock budget in milliseconds for one planning step before it yields and resumes on a
## later frame. [code]0[/code] disables time-slicing.
@export var time_slice_ms: int = 10
## Starting contents of the agent's own blackboard. The agent also gets an [code]entity[/code]
## property and a [code]GDPAI_OBJECTS[/code] list of its own object data.
@export var blackboard_plan: GdPAIBlackboardPlan = GdPAIBlackboardPlan.new()
## Behaviors applied to the agent when it becomes ready. Their goals, actions and updaters are
## all added to the agent.
@export var behavior_configs: Array[GdPAIBehaviorConfig] = []
