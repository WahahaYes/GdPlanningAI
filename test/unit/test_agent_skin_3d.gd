extends GutTest
## Agent skin instances a character as Body, retargets the shared Rig_Medium
## clips onto it, and follows body speed for idle/walk.

const AGENT_3D_PREFAB: PackedScene = preload("res://examples/source_3d/prefabs/agent_3d.tscn")
const BARBARIAN_SKIN: PackedScene = preload(
	"res://examples/source_3d/assets/vendored/KayKit_Adventurers/Characters/gltf/Barbarian.glb"
)


func after_each() -> void:
	var scheduler: GdPAIPlanScheduler = _scheduler()
	if scheduler != null:
		scheduler.cancel_all_jobs()
	await _drain_scheduler()


func _drain_scheduler(timeout_frames: int = 60) -> void:
	var scheduler: GdPAIPlanScheduler = _scheduler()
	if scheduler == null:
		return
	for i in range(timeout_frames):
		scheduler.process_callbacks()
		if scheduler.active_job_count() == 0:
			return
		await get_tree().process_frame


func _scheduler() -> GdPAIPlanScheduler:
	return GdPAIAutoload.get_scheduler()


func _pump_frames(frames: int) -> void:
	for i in range(frames):
		await get_tree().physics_frame
		await get_tree().process_frame


func _make_agent() -> Dictionary:
	var agent_entity: CharacterBody3D = AGENT_3D_PREFAB.instantiate() as CharacterBody3D
	add_child_autofree(agent_entity)
	var agent: GdPAIAgent = agent_entity.get_node("GdPAIAgent") as GdPAIAgent
	agent.set_planning_strategy(GdPAIAgentConfig.PlanningStrategy.ON_DEMAND)
	var skin: Node3D = agent_entity.get_node("Skin") as Node3D
	var player: AnimationPlayer = skin.get_node("AnimationPlayer") as AnimationPlayer
	return {"entity": agent_entity, "skin": skin, "player": player}


func test_skin_instances_knight_character() -> void:
	var setup: Dictionary = _make_agent()
	var skin: Node3D = setup["skin"]
	await _pump_frames(2)
	var body: Node3D = skin.get_node("Body") as Node3D
	assert_not_null(body, "Skin should instance the character as Body")
	assert_not_null(
		body.find_child("Knight_Body", true, false), "Default skin should contain the Knight meshes"
	)


func test_rig_clips_are_retargeted() -> void:
	var setup: Dictionary = _make_agent()
	var player: AnimationPlayer = setup["player"]
	await _pump_frames(2)
	assert_true(player.has_animation("Idle_A"), "General pack idle should be on the skin player")
	assert_true(
		player.has_animation("Walking_A"), "Movement pack walk should be on the skin player"
	)


func test_walk_plays_when_moving() -> void:
	var setup: Dictionary = _make_agent()
	var agent_entity: CharacterBody3D = setup["entity"]
	var player: AnimationPlayer = setup["player"]
	agent_entity.velocity = Vector3(3.0, 0.0, 0.0)
	await _pump_frames(3)
	assert_eq(player.current_animation, "Walking_A")


func test_locomotion_clips_loop() -> void:
	var setup: Dictionary = _make_agent()
	var player: AnimationPlayer = setup["player"]
	await _pump_frames(2)
	assert_eq(
		player.get_animation("Idle_A").loop_mode,
		Animation.LOOP_LINEAR,
		"Idle should loop (imports arrive LOOP_NONE)"
	)
	assert_eq(
		player.get_animation("Walking_A").loop_mode,
		Animation.LOOP_LINEAR,
		"Walk should loop (imports arrive LOOP_NONE)"
	)


func test_idle_plays_when_still() -> void:
	var setup: Dictionary = _make_agent()
	var player: AnimationPlayer = setup["player"]
	await _pump_frames(3)
	assert_eq(player.current_animation, "Idle_A")


func test_skin_swaps_at_runtime() -> void:
	var setup: Dictionary = _make_agent()
	var skin: Node3D = setup["skin"]
	var player: AnimationPlayer = setup["player"]
	skin.set_skin(BARBARIAN_SKIN)
	await _pump_frames(2)
	var body: Node3D = skin.get_node("Body") as Node3D
	assert_not_null(
		body.find_child("Barbarian_Body", true, false),
		"Swapped skin should contain the Barbarian meshes"
	)
	assert_null(
		body.find_child("Knight_Body", true, false), "Swapped skin should drop the Knight meshes"
	)
	assert_true(player.has_animation("Walking_A"), "Clips should survive a skin swap")
