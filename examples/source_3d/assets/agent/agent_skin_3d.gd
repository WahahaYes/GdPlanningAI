extends Node3D
## Swappable 3D agent skin: instances a KayKit Adventurers character as [code]Body[/code] and
## plays idle or walk from [member entity]'s horizontal speed. Presentation only.[br]
## All Adventurers share one skeleton, so the Rig_Medium clips are remapped once into a shared
## cache. Swap looks with [member skin_scene] or [method set_skin].

## Default Knight character visual.
const KNIGHT_SKIN: PackedScene = preload(
	"res://examples/source_3d/assets/vendored/KayKit_Adventurers/Characters/gltf/Knight.glb"
)
## General Rig_Medium clips (idle, pickup, interact).
const PACK_GENERAL: PackedScene = preload(
	# gdlint:ignore = max-line-length
	"res://examples/source_3d/assets/vendored/KayKit_Adventurers/Animations/gltf/Rig_Medium/Rig_Medium_General.glb"
)
## Movement Rig_Medium clips (walk, run, jump).
const PACK_MOVEMENT: PackedScene = preload(
	# gdlint:ignore = max-line-length
	"res://examples/source_3d/assets/vendored/KayKit_Adventurers/Animations/gltf/Rig_Medium/Rig_Medium_MovementBasic.glb"
)

## Remapped clips shared by every skin instance, built once from the packs.
static var _clip_cache: Dictionary = {}

## Character look instanced as Body. Any Rig_Medium `.glb` works.
@export var skin_scene: PackedScene = KNIGHT_SKIN
## Body whose horizontal speed drives locomotion.
@export var entity: CharacterBody3D
## Scale applied to the instanced character.
@export var skin_scale: float = 0.6
## Idle clip name from the general pack.
@export var idle_animation: String = "Idle_A"
## Locomotion clip name from the movement pack.
@export var walk_animation: String = "Walking_A"
## Horizontal speed in m/s above which the walk clip plays.
@export var walk_threshold: float = 0.4
## Crossfade time in seconds when switching clips.
@export var blend_time: float = 0.25

## Player holding the retargeted clips.
var _player: AnimationPlayer = null
## Currently playing clip.
var _current: String = ""


## Builds the player once and instances the default skin.
func _ready() -> void:
	_build_clip_cache()
	_loop_locomotion_clips()
	_player = AnimationPlayer.new()
	_player.name = "AnimationPlayer"
	add_child(_player)
	_player.root_node = _player.get_path_to(self)
	var library: AnimationLibrary = AnimationLibrary.new()
	for clip_name in _clip_cache.keys():
		library.add_animation(clip_name, _clip_cache[clip_name])
	_player.add_animation_library("", library)
	set_skin(skin_scene)


## Switches clips from body speed.
func _process(_delta: float) -> void:
	if _player == null or not is_instance_valid(entity):
		return
	var flat: Vector3 = entity.velocity
	flat.y = 0.0
	if flat.length() > walk_threshold:
		_play(walk_animation)
	else:
		_play(idle_animation)


## Replaces the Body child with [param pack]. Safe to call at runtime.
func set_skin(pack: PackedScene) -> void:
	skin_scene = pack
	if _player == null:
		return
	var old: Node = get_node_or_null("Body")
	if old != null:
		remove_child(old)
		old.queue_free()
	var body: Node3D = pack.instantiate() as Node3D
	body.name = "Body"
	body.scale = Vector3.ONE * skin_scale
	add_child(body)
	move_child(body, 0)
	_current = ""
	_play(idle_animation)


## Forces looping on idle and walk; imported Rig_Medium clips arrive as LOOP_NONE.
func _loop_locomotion_clips() -> void:
	for clip_name in [idle_animation, walk_animation]:
		if _clip_cache.has(clip_name):
			(_clip_cache[clip_name] as Animation).loop_mode = Animation.LOOP_LINEAR


## Plays [param clip] when it differs from the current one.
func _play(clip: String) -> void:
	if clip == _current or not _player.has_animation(clip):
		return
	_current = clip
	_player.play(clip, blend_time)


## Copies every pack clip once, prefixed to the runtime Body path.
func _build_clip_cache() -> void:
	if not _clip_cache.is_empty():
		return
	var packs: Array[PackedScene] = [PACK_GENERAL, PACK_MOVEMENT]
	for pack in packs:
		var rig: Node = pack.instantiate()
		var source: AnimationPlayer = (
			rig.find_child("AnimationPlayer", true, false) as AnimationPlayer
		)
		if source != null:
			for clip_name in source.get_animation_list():
				if _clip_cache.has(clip_name):
					continue
				var remapped: Animation = source.get_animation(clip_name).duplicate() as Animation
				for track in range(remapped.get_track_count()):
					var old_path: NodePath = remapped.track_get_path(track)
					remapped.track_set_path(track, "Body/" + str(old_path))
				_clip_cache[clip_name] = remapped
		rig.queue_free()
