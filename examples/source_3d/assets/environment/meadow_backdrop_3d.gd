@tool
extends Node3D
## Scenery around a rectangular play area: wind-blown grass over it, a tree line just outside
## it and hills and mountains further out, so the camera never sees the edge of the world.[br]
## Built from a seed into MultiMeshes on load, in the editor too. The generated nodes are not
## saved with the scene. Presentation only: nothing here collides or touches navigation.

## Group of nodes (campfire clearings) that keep grass away within [member clearing_radius].
const CLEARING_GROUP: StringName = &"meadow_clearing"

## Size of the walkable ground on X and Z, centred on this node.
@export var play_area_size: Vector2 = Vector2(40.0, 40.0)
## Height of the walkable ground's top surface.
@export var ground_height: float = 0.25
## Seed for every random placement; change it for a different layout.
@export var layout_seed: int = 1

@export_group("Grass")
## Material for the blades (meadow_grass_3d.tres). Null grows no grass.
@export var grass_material: Material
## Grass clumps per square metre where the patch noise allows grass.
@export var grass_density: float = 12.0
## How far the grass reaches past the play area, under the tree line.
@export var grass_margin: float = 4.0
## Radius kept bare around each node in [constant CLEARING_GROUP].
@export var clearing_radius: float = 1.2

@export_group("Tree line")
## Tree models scattered along the tree line.
@export var tree_scenes: Array[PackedScene] = []
## Distance band outside the play area edge that trees fill.
@export var tree_band: Vector2 = Vector2(2.0, 14.0)
## Grid step, in metres, between tree line positions before jitter.
@export var tree_spacing: float = 3.4
## Tree scale at the front of the band and at the back.
@export var tree_scale: Vector2 = Vector2(1.8, 3.6)

@export_group("Horizon")
## Hills and mountains placed on a ring around the play area.
@export var horizon_scenes: Array[PackedScene] = []
## Inner and outer radius of the horizon ring.
@export var horizon_radius: Vector2 = Vector2(60.0, 110.0)
## Number of hills and mountains on the ring.
@export var horizon_count: int = 28
## Scale range of the hills and mountains.
@export var horizon_scale: Vector2 = Vector2(9.0, 16.0)

@export_group("Clouds")
## Cloud models floating on a ring above the horizon.
@export var cloud_scenes: Array[PackedScene] = []
## Replaces the clouds' own material, e.g. one that ignores fog so distant clouds stay white.
@export var cloud_material: Material
## Number of clouds on the ring.
@export var cloud_count: int = 14
## Inner and outer radius of the cloud ring.
@export var cloud_radius: Vector2 = Vector2(60.0, 110.0)
## Height range of the clouds above the ground.
@export var cloud_height: Vector2 = Vector2(22.0, 38.0)
## Scale range of the clouds.
@export var cloud_scale: Vector2 = Vector2(5.0, 9.0)
## Drift of the whole cloud ring, in degrees per second.
@export var cloud_drift: float = 0.4

var _rng: RandomNumberGenerator = RandomNumberGenerator.new()


func _ready() -> void:
	# Deferred so instanced campfires have joined CLEARING_GROUP.
	_build.call_deferred()


## Regenerates every scenery node from the current settings.
func _build() -> void:
	for child in get_children():
		if child.has_meta(&"meadow_generated"):
			child.queue_free()
	_rng.seed = layout_seed
	_build_grass()
	_build_tree_line()
	_build_horizon()
	_build_clouds()


func _process(delta: float) -> void:
	var clouds: Node3D = get_node_or_null(^"Clouds") as Node3D
	if clouds != null:
		clouds.rotate_y(deg_to_rad(cloud_drift) * delta)


func _build_grass() -> void:
	if grass_material == null or grass_density <= 0.0:
		return
	var noise: FastNoiseLite = FastNoiseLite.new()
	noise.seed = layout_seed
	noise.frequency = 0.08
	var clearings: Array[Vector2] = []
	for node in get_tree().get_nodes_in_group(CLEARING_GROUP):
		if node is Node3D:
			var p: Vector3 = to_local((node as Node3D).global_position)
			clearings.append(Vector2(p.x, p.z))
	var half: Vector2 = play_area_size * 0.5 + Vector2(grass_margin, grass_margin)
	var attempts: int = int(half.x * half.y * 4.0 * grass_density)
	var transforms: Array[Transform3D] = []
	for i in attempts:
		var p: Vector2 = Vector2(
			_rng.randf_range(-half.x, half.x), _rng.randf_range(-half.y, half.y)
		)
		# Patchy cover: thick tufts in some places, bare ground between.
		if noise.get_noise_2dv(p) + 0.75 < _rng.randf():
			continue
		if _in_clearing(p, clearings):
			continue
		var basis: Basis = Basis(Vector3.UP, _rng.randf() * TAU).scaled(
			Vector3.ONE * _rng.randf_range(0.7, 1.3)
		)
		transforms.append(Transform3D(basis, Vector3(p.x, ground_height, p.y)))
	var mmi: MultiMeshInstance3D = _multimesh(_grass_clump_mesh(), transforms, "Grass")
	mmi.material_override = grass_material
	mmi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF


func _build_tree_line() -> void:
	if tree_scenes.is_empty():
		return
	var half: Vector2 = play_area_size * 0.5
	var reach: float = tree_band.y
	var placements: Array = []
	var x: float = -half.x - reach
	while x <= half.x + reach:
		var z: float = -half.y - reach
		while z <= half.y + reach:
			var p: Vector2 = Vector2(x, z) + _jitter(tree_spacing * 0.45)
			var outside: float = maxf(absf(p.x) - half.x, absf(p.y) - half.y)
			# Thinner near the play area, dense further back.
			var fill: float = inverse_lerp(tree_band.x, tree_band.x + 4.0, outside)
			if outside >= tree_band.x and outside <= reach and _rng.randf() < 0.35 + fill:
				# Smaller trees at the front so they hide less of the play area.
				var s: float = lerpf(tree_scale.x, tree_scale.y, clampf(fill, 0.0, 1.0))
				placements.append([_rng.randi() % tree_scenes.size(), p, Vector2(s * 0.85, s)])
			z += tree_spacing
		x += tree_spacing
	_place_scenes(tree_scenes, placements, "TreeLine", true)


func _build_horizon() -> void:
	if horizon_scenes.is_empty() or horizon_count <= 0:
		return
	var placements: Array = []
	for i in horizon_count:
		var angle: float = TAU * (float(i) + _rng.randf_range(-0.3, 0.3)) / float(horizon_count)
		var radius: float = _rng.randf_range(horizon_radius.x, horizon_radius.y)
		var p: Vector2 = Vector2(cos(angle), sin(angle)) * radius
		placements.append([_rng.randi() % horizon_scenes.size(), p, horizon_scale])
	_place_scenes(horizon_scenes, placements, "Horizon", false)


func _build_clouds() -> void:
	if cloud_scenes.is_empty() or cloud_count <= 0:
		return
	var ring: Node3D = Node3D.new()
	ring.name = "Clouds"
	ring.set_meta(&"meadow_generated", true)
	add_child(ring)
	var placements: Array = []
	for i in cloud_count:
		var angle: float = TAU * (float(i) + _rng.randf_range(-0.4, 0.4)) / float(cloud_count)
		var radius: float = _rng.randf_range(cloud_radius.x, cloud_radius.y)
		var p: Vector2 = Vector2(cos(angle), sin(angle)) * radius
		var h: float = _rng.randf_range(cloud_height.x, cloud_height.y)
		placements.append([_rng.randi() % cloud_scenes.size(), p, cloud_scale, h])
	_place_scenes(cloud_scenes, placements, "Cloud", false, ring)
	if cloud_material != null:
		for mmi in ring.get_children():
			(mmi as GeometryInstance3D).material_override = cloud_material


## Places [code][scene index, position, scale range, height?][/code] entries as one MultiMesh
## per mesh, under [param parent] (this node by default).
func _place_scenes(
	scenes: Array[PackedScene], placements: Array, label: String, shadows: bool, parent: Node = self
) -> void:
	for index in scenes.size():
		var parts: Array = _mesh_parts(scenes[index])
		for part_index in parts.size():
			var mesh: Mesh = parts[part_index][0]
			var offset: Transform3D = parts[part_index][1]
			var transforms: Array[Transform3D] = []
			for entry in placements:
				if entry[0] != index:
					continue
				var p: Vector2 = entry[1]
				var scale_range: Vector2 = entry[2]
				var s: float = _rng.randf_range(scale_range.x, scale_range.y)
				var basis: Basis = Basis(Vector3.UP, _rng.randf() * TAU).scaled(Vector3.ONE * s)
				var y: float = ground_height + (float(entry[3]) if entry.size() > 3 else 0.0)
				transforms.append(Transform3D(basis, Vector3(p.x, y, p.y)) * offset)
			if transforms.is_empty():
				continue
			var mmi: MultiMeshInstance3D = _multimesh(
				mesh, transforms, "%s%d_%d" % [label, index, part_index], parent
			)
			if not shadows:
				mmi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF


## Returns [code][mesh, transform relative to the root][/code] for each mesh in a scene.
func _mesh_parts(scene: PackedScene) -> Array:
	var parts: Array = []
	if scene == null:
		return parts
	var root: Node = scene.instantiate() as Node
	for node in root.find_children("*", "MeshInstance3D", true, false):
		var mi: MeshInstance3D = node as MeshInstance3D
		var offset: Transform3D = Transform3D.IDENTITY
		var walker: Node = mi
		while walker != root and walker is Node3D:
			offset = (walker as Node3D).transform * offset
			walker = walker.get_parent()
		parts.append([mi.mesh, offset])
	root.free()
	return parts


func _multimesh(
	mesh: Mesh, transforms: Array[Transform3D], node_name: String, parent: Node = self
) -> MultiMeshInstance3D:
	var multimesh: MultiMesh = MultiMesh.new()
	multimesh.transform_format = MultiMesh.TRANSFORM_3D
	multimesh.mesh = mesh
	multimesh.instance_count = transforms.size()
	for i in transforms.size():
		multimesh.set_instance_transform(i, transforms[i])
	var mmi: MultiMeshInstance3D = MultiMeshInstance3D.new()
	mmi.name = node_name
	mmi.multimesh = multimesh
	mmi.set_meta(&"meadow_generated", true)
	parent.add_child(mmi)
	return mmi


## A tuft of five tapered blades. UV.y runs from 0 at the root to 1 at the tip.
func _grass_clump_mesh() -> ArrayMesh:
	var st: SurfaceTool = SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)
	st.set_normal(Vector3.UP)
	var blade_rng: RandomNumberGenerator = RandomNumberGenerator.new()
	blade_rng.seed = layout_seed
	for i in 5:
		var yaw: float = TAU * float(i) / 5.0 + blade_rng.randf_range(-0.4, 0.4)
		var side: Vector3 = Vector3(cos(yaw), 0.0, sin(yaw))
		var root: Vector3 = Vector3(sin(yaw), 0.0, -cos(yaw)) * blade_rng.randf_range(0.0, 0.08)
		var lean: Vector3 = root.normalized() * blade_rng.randf_range(0.03, 0.08)
		var tip: Vector3 = root + lean + Vector3.UP * blade_rng.randf_range(0.14, 0.26)
		var width: float = blade_rng.randf_range(0.03, 0.05)
		st.set_uv(Vector2(0.0, 0.0))
		st.add_vertex(root - side * width)
		st.set_uv(Vector2(1.0, 0.0))
		st.add_vertex(root + side * width)
		st.set_uv(Vector2(0.5, 1.0))
		st.add_vertex(tip)
	return st.commit()


func _in_clearing(p: Vector2, clearings: Array[Vector2]) -> bool:
	for c in clearings:
		if p.distance_squared_to(c) < clearing_radius * clearing_radius:
			return true
	return false


func _jitter(amount: float) -> Vector2:
	return Vector2(_rng.randf_range(-amount, amount), _rng.randf_range(-amount, amount))
