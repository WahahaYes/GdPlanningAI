extends OmniLight3D
## Flickers a fire's light; its energy, reach and flicker follow the flame's size (the fuel
## level, eased by the campfire runtime). Presentation only.

## The flame node whose scale (the fuel level) drives the light. Optional.
@export var flame: Node3D
## Energy at full fuel, before flicker.
@export var base_energy: float = 3.2
## Fraction of the energy that flickers.
@export_range(0.0, 1.0) var flicker: float = 0.35
## How fast the flicker changes.
@export var flicker_speed: float = 9.0
## Light range at an empty and at a full fire.
@export var range_span: Vector2 = Vector2(3.0, 8.0)

var _noise: FastNoiseLite = FastNoiseLite.new()
var _time: float = 0.0
var _rest: Vector3


func _ready() -> void:
	_noise.seed = randi()
	_noise.frequency = 1.0
	_rest = position


func _process(delta: float) -> void:
	_time += delta * flicker_speed
	var n: float = _noise.get_noise_1d(_time)
	var fuel: float = flame.scale.x if flame != null else 1.0
	light_energy = base_energy * (0.25 + 0.75 * fuel) * (1.0 + n * flicker * (0.5 + fuel))
	omni_range = lerpf(range_span.x, range_span.y, fuel)
	# A little sway sells the flicker on nearby shadows.
	position = (
		_rest
		+ Vector3(_noise.get_noise_1d(_time + 40.0), 0.0, _noise.get_noise_1d(_time + 80.0)) * 0.06
	)
