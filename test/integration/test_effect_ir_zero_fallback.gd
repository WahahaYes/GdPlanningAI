extends "res://test/shared/scene_smoke_base.gd"
## Every built-in action declares a native effect spec, so planning the large campfire scene
## must never round-trip an effect through GDScript. Cost and custom-precondition callables
## still run; only ApplyEffect dispatches are counted, and they must stay at zero.

const LARGE_SCENE: PackedScene = preload("res://examples/campfire_large_3d.tscn")


## Plans the large scene unparked, then requires zero effect callbacks with IR hits.
func test_effect_ir_serves_every_effect_without_callbacks() -> void:
	var scheduler: GdPAIPlanScheduler = _scheduler()
	assert_not_null(scheduler, "Autoload scheduler must exist")
	var before: Dictionary = scheduler.get_effect_dispatch_stats()

	var scene: Node3D = LARGE_SCENE.instantiate() as Node3D
	add_child_autofree(scene)

	# Agents plan on their interval; wait until native effects have run.
	var ir_seen: int = 0
	for i in range(600):
		scheduler.process_callbacks()
		await get_tree().process_frame
		var now: Dictionary = scheduler.get_effect_dispatch_stats()
		ir_seen = int(now["effect_ir_hits"]) - int(before["effect_ir_hits"])
		if ir_seen >= 20:
			break
	assert_gt(ir_seen, 0, "Native IR should have applied effects while agents plan")

	scheduler.cancel_all_jobs()
	await _drain_scheduler()
	var after: Dictionary = scheduler.get_effect_dispatch_stats()
	assert_eq(
		int(after["effect_callback_fallbacks"]) - int(before["effect_callback_fallbacks"]),
		0,
		"No ApplyEffect GDScript callbacks while IR covers every action"
	)
	assert_gt(
		int(after["effect_ir_hits"]) - int(before["effect_ir_hits"]),
		0,
		"Native IR served effects during the scene run"
	)
