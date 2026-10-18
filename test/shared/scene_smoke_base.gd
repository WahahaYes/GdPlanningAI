extends GutTest
## Shared smoke helpers for example scenes.
##[br]
##[br]
## Example scenes are demos, not contracts: tests only prove a scene
## instantiates, settles a few frames, and still contains its world and
## agents. Exact plans, counts, layouts, and visuals are left unpinned.


## Cancels background planning jobs after each test.
func after_each() -> void:
	var scheduler: GdPAIPlanScheduler = _scheduler()
	if scheduler != null:
		scheduler.cancel_all_jobs()
	await _drain_scheduler()


## Returns the shared planning scheduler.
func _scheduler() -> GdPAIPlanScheduler:
	return GdPAIAutoload.get_scheduler()


## Pumps the tree so physics, spawners, and scripts can settle.
func _pump_frames(frames: int) -> void:
	for i in range(frames):
		await get_tree().physics_frame
		await get_tree().process_frame


## Waits for in-flight planning jobs to finish.
func _drain_scheduler(timeout_frames: int = 60) -> void:
	var scheduler: GdPAIPlanScheduler = _scheduler()
	if scheduler == null:
		return
	for i in range(timeout_frames):
		scheduler.process_callbacks()
		if scheduler.active_job_count() == 0:
			return
		await get_tree().process_frame


## Parks every agent in the scene so settling never starts live planning.
func _park_agents(scene: Node) -> void:
	for agent_node in scene.find_children("*", "GdPAIAgent", true, false):
		var agent: GdPAIAgent = agent_node as GdPAIAgent
		if agent != null:
			agent.set_planning_strategy(GdPAIAgentConfig.PlanningStrategy.ON_DEMAND)


## Adds the scene, parks agents (including spawner output), and settles.
func _settle_example(scene: Node, frames: int = 10) -> void:
	_park_agents(scene)
	add_child_autofree(scene)
	await _pump_frames(2)
	_park_agents(scene)
	await _pump_frames(frames)
