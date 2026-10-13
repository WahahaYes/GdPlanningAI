extends Node
## Spawns a configurable number of agents scattered in a ground box.
## 3D counterpart of many_agents_spawner.
##[br]
##[br]
## Add this node to a scene, assign [member agent_scene], and set
## [member agent_count]. Agents rest on [member spawn_height].

## Agent packed scene to instantiate.
@export var agent_scene: PackedScene
## Number of agents to spawn.
@export var agent_count: int = 6
## Minimum corner of the spawn volume.
@export var spawn_min: Vector3 = Vector3(-16, 0.25, -16)
## Maximum corner of the spawn volume.
@export var spawn_max: Vector3 = Vector3(16, 0.25, 16)
## When true, hides each agent's Label3D to reduce clutter at scale.
@export var hide_debug_labels: bool = false
## Character looks cycled over spawned agents by spawn index. Empty keeps prefab defaults.
@export var skins: Array[PackedScene] = []
## Agents sharing each home campfire, assigned in spawn order to the scene's
## campfires in tree order. 0 leaves every agent free to tend any campfire, as do
## agents beyond the last campfire.
@export var agents_per_campfire: int = 0
## Index of the spawned agent a camera rig should follow (-1 assigns none).
@export var camera_agent_index: int = 0
## Path to a follow rig whose target accepts a spawned agent.
@export var camera_rig_path: NodePath


## Scatters the agents on ready.
func _ready() -> void:
	if agent_scene == null:
		push_error("ManyAgentsSpawner3D: agent_scene is not set.")
		return
	var agents: Array[Node3D] = []
	for i in agent_count:
		var agent: Node3D = agent_scene.instantiate() as Node3D
		agent.name = "Agent%d" % (i + 1)
		agent.position = Vector3(
			randf_range(spawn_min.x, spawn_max.x),
			randf_range(spawn_min.y, spawn_max.y),
			randf_range(spawn_min.z, spawn_max.z),
		)
		add_child(agent)
		if not skins.is_empty():
			var skin: Node = agent.get_node_or_null("Skin")
			if skin != null and skin.has_method("set_skin"):
				skin.call("set_skin", skins[i % skins.size()])
		if hide_debug_labels:
			var label: Node = agent.get_node_or_null("Label3D")
			if label != null:
				label.set("visible", false)
		agents.append(agent)
	_assign_home_campfires(agents)
	if camera_agent_index >= 0 and camera_agent_index < agents.size():
		var rig: Node = get_node_or_null(camera_rig_path)
		if rig != null:
			rig.set("target", agents[camera_agent_index])


## Gives each block of [member agents_per_campfire] agents its own campfire: the
## campfire joins a [code]HomeCampfireN[/code] group and the agents' fire goal tends it.
func _assign_home_campfires(agents: Array[Node3D]) -> void:
	if agents_per_campfire <= 0:
		return
	var scene: Node = owner if owner != null else get_tree().current_scene
	if scene == null:
		return
	var campfires: Array[Node] = scene.find_children("*", "CampfireObject", true, false)
	for i in agents.size():
		var home: int = i / agents_per_campfire
		if home >= campfires.size():
			return
		var group: String = "HomeCampfire%d" % (home + 1)
		campfires[home].add_to_group(group)
		for agent_node in agents[i].find_children("*", "GdPAIAgent", true, false):
			for goal in (agent_node as GdPAIAgent).goals:
				if goal is MaintainFireGoal:
					(goal as MaintainFireGoal).campfire_group = group
