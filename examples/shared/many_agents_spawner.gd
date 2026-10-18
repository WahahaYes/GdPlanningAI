extends Node
## Spawns [member agent_count] copies of [member agent_scene] at random points in
## [member spawn_rect]. Used by the stress-test scene; locks rotation so sprites stay upright,
## and can hide debug labels so UI cost does not skew planner timing.

## Agent packed scene to instantiate.
@export var agent_scene: PackedScene
## Number of agents to spawn.
@export var agent_count: int = 20
## World-space rectangle within which agents are randomly placed.
@export var spawn_rect: Rect2 = Rect2(-800, -1800, 2400, 2200)
## When true, hides each agent's DebugLabel to reduce UI overhead at scale.
@export var hide_debug_labels: bool = true


## Scatters the agents on ready.
func _ready() -> void:
	if agent_scene == null:
		push_error("ManyAgentsSpawner: agent_scene is not set.")
		return
	for i in agent_count:
		var agent: Node = agent_scene.instantiate()
		agent.position = Vector2(
			randf_range(spawn_rect.position.x, spawn_rect.end.x),
			randf_range(spawn_rect.position.y, spawn_rect.end.y),
		)
		agent.lock_rotation = true
		add_child(agent)
		if hide_debug_labels:
			var label: Node = agent.get_node_or_null("DebugLabel")
			if label != null:
				label.hide()
