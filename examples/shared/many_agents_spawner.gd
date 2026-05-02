extends Node
## Spawns a configurable number of agents at random positions within [member spawn_rect].
##[br]
##[br]
## Add this node to a scene, assign [member agent_scene], and set [member agent_count].
## Agents are scattered randomly inside [member spawn_rect] on [method _ready].

## Agent packed scene to instantiate.
@export var agent_scene: PackedScene
## Number of agents to spawn.
@export var agent_count: int = 20
## World-space rectangle within which agents are randomly placed.
@export var spawn_rect: Rect2 = Rect2(-800, -1800, 2400, 2200)
## When true, hides each agent's DebugLabel to reduce UI overhead at scale.
@export var hide_debug_labels: bool = true


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
