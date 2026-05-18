extends GutTest

const AGENT_PREFAB: PackedScene = preload("res://examples/source_2d/prefabs/agent.tscn")
const BANANA_TREE_PREFAB: PackedScene = preload("res://examples/source_2d/prefabs/banana_tree.tscn")
const SCENERY_PREFAB: PackedScene = preload("res://examples/source_2d/prefabs/scenery.tscn")
const WORLD_NODE_SCRIPT: Script = preload(
	"res://addons/GdPlanningAI/scripts/nodes/gdpai_world_node.gd"
)
const BLACKBOARD_PLAN_SCRIPT: Script = preload(
	"res://addons/GdPlanningAI/scripts/gdpai_blackboard_plan.gd"
)


func after_each() -> void:
	for node in get_tree().get_nodes_in_group("GdPAIObjectData"):
		if node is FoodObject and is_instance_valid(node.entity):
			node.entity.queue_free()
	await _drain_scheduler()


func _drain_scheduler(timeout_frames: int = 120) -> void:
	var scheduler: GdPAIPlanScheduler = _scheduler()
	if scheduler == null:
		return
	for i in range(timeout_frames):
		scheduler.process_callbacks()
		if scheduler.active_job_count() == 0:
			return
		await get_tree().process_frame


func _make_world_node() -> GdPAIWorldNode:
	var world_node: GdPAIWorldNode = WORLD_NODE_SCRIPT.new()
	world_node.name = "GdPAIWorldNode"
	world_node.blackboard_plan = BLACKBOARD_PLAN_SCRIPT.new()
	add_child_autofree(world_node)
	return world_node


func _pump_frames(frames: int) -> void:
	for i in range(frames):
		await get_tree().physics_frame
		await get_tree().process_frame


func _scheduler() -> GdPAIPlanScheduler:
	return GdPAIAutoload.get_scheduler()


func _wait_for_plan(agent: GdPAIAgent, timeout_frames: int = 180) -> Array[Action]:
	var scheduler: GdPAIPlanScheduler = _scheduler()
	for i in range(timeout_frames):
		scheduler.process_callbacks()
		var plan: Array[Action] = agent.get_current_plan()
		if not plan.is_empty():
			return plan
		await get_tree().process_frame
	fail_test("Timed out waiting for agent plan")
	return []


func _start_plan_and_wait(agent: GdPAIAgent, timeout_frames: int = 180) -> Array[Action]:
	var scheduler: GdPAIPlanScheduler = _scheduler()
	var previous_plan: Array[Action] = agent.get_current_plan()
	agent.manually_start_plan()
	var saw_job: bool = scheduler.active_job_count() > 0
	for i in range(timeout_frames):
		scheduler.process_callbacks()
		saw_job = saw_job or scheduler.active_job_count() > 0
		if saw_job and scheduler.active_job_count() == 0:
			return agent.get_current_plan()
		if agent.get_current_plan() != previous_plan:
			return agent.get_current_plan()
		await get_tree().process_frame
	fail_test("Timed out waiting for submitted agent plan")
	return []


func _plan_titles(plan: Array[Action]) -> String:
	var titles: Array[String] = []
	for action in plan:
		titles.append(action.get_title())
	return " -> ".join(titles)


func _wait_for_fruit(timeout_frames: int = 180) -> FoodObject:
	for i in range(timeout_frames):
		for node in get_tree().get_nodes_in_group("GdPAIObjectData"):
			if node is FoodObject:
				return node
		await get_tree().process_frame
	fail_test("Timed out waiting for dropped fruit")
	return null


func _finish_current_plan(agent: GdPAIAgent, timeout_frames: int = 300) -> void:
	for i in range(timeout_frames):
		_scheduler().process_callbacks()
		if agent.get_current_plan_step() > agent.get_current_plan().size():
			return
		agent.call("_execute_plan", 0.1)
		await get_tree().physics_frame
		await get_tree().process_frame
	fail_test("Timed out waiting for current plan to finish")


func test_real_hunger_example_shakes_tree_then_picks_up_food() -> void:
	_make_world_node()

	var scenery: Node2D = SCENERY_PREFAB.instantiate()
	add_child_autofree(scenery)
	for node in scenery.find_children("*", "FruitTreeObject", true, false):
		node.entity.queue_free()

	var tree: Node2D = BANANA_TREE_PREFAB.instantiate()
	add_child_autofree(tree)
	tree.global_position = Vector2(281, 89)
	var fruit_tree: FruitTreeObject = GdPAIUTILS.get_child_of_type(tree, FruitTreeObject)
	fruit_tree.drop_min_amount = 1
	fruit_tree.drop_max_amount = 1
	fruit_tree.drop_distance = 0.0

	var agent_entity: Node2D = AGENT_PREFAB.instantiate()
	add_child_autofree(agent_entity)
	agent_entity.global_position = Vector2(120, 160)
	var agent: GdPAIAgent = GdPAIUTILS.get_child_of_type(agent_entity, GdPAIAgent)
	agent.config.planning_strategy = GdPAIAgentConfig.PlanningStrategy.ON_DEMAND
	await _pump_frames(3)

	agent.blackboard.set_property("hunger", 0.0)
	var first_plan: Array[Action] = await _start_plan_and_wait(agent)
	assert_false(first_plan.is_empty(), "Agent should plan while full")
	assert_eq(first_plan[0].get_title(), "Wander", "Agent should wander while hunger is low")

	agent.blackboard.set_property("hunger", 30.0)
	var shake_plan: Array[Action] = await _start_plan_and_wait(agent)
	assert_false(shake_plan.is_empty(), "Agent should plan when hungry")
	assert_eq(shake_plan.size(), 2, "Plan should have GoTo → Shake Tree chain")
	assert_eq(shake_plan[0].get_title(), "Go To", "First action should be GoTo")
	assert_eq(shake_plan[1].get_title(), "Shake Tree", "Second action should be Shake Tree")
	await _finish_current_plan(agent)

	var food: FoodObject = await _wait_for_fruit()
	assert_not_null(food, "Shake Tree should drop a real FoodObject")

	# Wait extra frames to ensure world state reflects the new fruit
	await _pump_frames(10)

	var world_actions: Array[Action] = agent._collect_worldly_actions()
	var action_titles: Array[String] = []
	for wa in world_actions:
		action_titles.append(wa.get_title())
	assert_true(action_titles.has("Pick Up Item"), "Agent should see world actions from dropped fruit (bananas), found: %s" % str(action_titles))

	agent.blackboard.set_property("hunger", 30.0)
	var pickup_plan: Array[Action] = await _start_plan_and_wait(agent)
	assert_false(pickup_plan.is_empty(), "Agent should plan after food drops")
	if pickup_plan.is_empty():
		return
	# New pattern: GoTo → PickupAction → EatHeldFoodAction chain
	assert_eq(pickup_plan.size(), 3, "Plan should have GoTo → Pickup → Eat chain")
	assert_eq(pickup_plan[0].get_title(), "Go To", "First action should be GoTo")
	assert_eq(pickup_plan[1].get_title(), "Pick Up Item", "Second action should be Pickup")
	assert_eq(pickup_plan[2].get_title(), "Eat Held Food", "Third action should be Eat")
