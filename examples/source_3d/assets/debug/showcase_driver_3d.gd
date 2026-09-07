extends Node3D
## Debug driver for the agent showcase scene: gives each child agent a skin
## and held item by index, parks planning, and keeps them wandering.
##[br]
##[br]
## Agents are discovered via their [GdPAIAgent] nodes, so the scene stays a
## plain list of prefab instances with no per-agent override blocks.

## Character looks cycled over the agents.
@export var skins: Array[PackedScene] = []
## Held_item ids cycled over the agents.
@export var held_items: Array[String] = ["potato", "wood", ""]
## Half-extent in meters of the wander square around the origin.
@export var wander_radius: float = 7.0
## Seconds between new wander targets.
@export var retarget_interval: float = 6.0
## Chance each arrival or retarget pauses instead of walking on.
@export var idle_chance: float = 0.35
## Shortest idle pause in seconds.
@export var idle_min_time: float = 1.5
## Longest idle pause in seconds.
@export var idle_max_time: float = 4.0

## One entry per agent: nav agent, countdown, and pause flag.
var _entries: Array[Dictionary] = []


## Assigns skins, held items, and first targets.
func _ready() -> void:
	var agent_nodes: Array[Node] = find_children("*", "GdPAIAgent", true, false)
	var index: int = 0
	for agent_node in agent_nodes:
		var agent: GdPAIAgent = agent_node as GdPAIAgent
		var entity: Node3D = agent.get_parent() as Node3D
		agent.set_planning_strategy(GdPAIAgentConfig.PlanningStrategy.ON_DEMAND)
		if not skins.is_empty():
			var skin: Node = entity.get_node_or_null("Skin")
			if skin != null and skin.has_method("set_skin"):
				skin.call("set_skin", skins[index % skins.size()])
		if not held_items.is_empty():
			agent.blackboard.set_property("held_item", held_items[index % held_items.size()])
		var nav: NavigationAgent3D = (
			entity.get_node_or_null("NavigationAgent3D") as NavigationAgent3D
		)
		_entries.append(
			{"nav": nav, "entity": entity, "timer": 0.0, "idling": false, "first": true}
		)
		index += 1


## Retargets finished agents; idlers wait out their pause first.
func _physics_process(delta: float) -> void:
	for entry in _entries:
		var nav: NavigationAgent3D = entry["nav"] as NavigationAgent3D
		if nav == null:
			continue
		var timer: float = float(entry["timer"]) - delta
		if bool(entry["idling"]):
			if timer <= 0.0:
				_decide(entry)
			else:
				entry["timer"] = timer
		elif timer <= 0.0 or nav.is_navigation_finished():
			_decide(entry)
		else:
			entry["timer"] = timer


## Walks on with a fresh target or pauses, then rearms the countdown.
## The first decision always walks so agents leave their spawn points.
## Pausing parks the nav target under the body so the controller eases to
## a stop instead of chasing the previous target through the pause.
func _decide(entry: Dictionary) -> void:
	var nav: NavigationAgent3D = entry["nav"] as NavigationAgent3D
	var first: bool = bool(entry.get("first", false))
	entry["first"] = false
	if not first and randf() < idle_chance:
		entry["idling"] = true
		entry["timer"] = randf_range(idle_min_time, idle_max_time)
		nav.target_position = (entry["entity"] as Node3D).global_position
		return
	entry["idling"] = false
	nav.target_position = Vector3(
		randf_range(-wander_radius, wander_radius), 0.75, randf_range(-wander_radius, wander_radius)
	)
	entry["timer"] = retarget_interval
