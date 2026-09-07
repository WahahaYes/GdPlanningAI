extends Node3D
## Smooth follow rig for the 3D demo camera.
##
## Position lerps toward [member target] each frame so the [Camera3D]
## riding the [SpringArm3D] child tracks the agent without jitter.
## Pitch/yaw live on the SpringArm; zoom via its spring length.

## The node to follow (the Agent1 CharacterBody3D).
@export var target: Node3D
## Height above the target origin.
@export var height: float = 1.2
## Position catch-up speed.
@export var follow_speed: float = 5.0


## Resolves the default agent sibling when no target is assigned.
func _ready() -> void:
	if target == null:
		var scene: Node = get_tree().current_scene
		if scene != null:
			target = scene.get_node_or_null("Agent1") as Node3D
	if target == null:
		target = get_node_or_null("../Agent1") as Node3D


## Tracks the target with frame-rate independent smoothing.
func _process(delta: float) -> void:
	if target == null:
		return
	var goal: Vector3 = target.global_position + Vector3(0.0, height, 0.0)
	var rate: float = clampf(follow_speed * delta, 0.0, 1.0)
	global_position = global_position.lerp(goal, rate)
