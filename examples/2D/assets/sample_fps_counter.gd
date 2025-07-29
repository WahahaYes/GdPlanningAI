## Simple FPS counter for monitoring the performance in GdPAI demos.

extends Node

## Reference to a label for displaying fps results.
@export var display_text: Label
## How long of a window to store FPS data.
@export var window_size: int = 180
## Array to store fps values.
@onready var fps_window: Array[float] = []
## What frame is running?
var frame_idx: int


func _process(delta: float) -> void:
	var fps_current: float = 1 / delta
	fps_window.append(fps_current)
	if fps_window.size() > window_size:
		fps_window.pop_front()

	var sum: float = 0
	for n in fps_window:
		sum += n  # Ensure n is treated as a float for accurate division
	var mean: float = sum / fps_window.size()
	var spike: float = fps_window.min()

	var variance: float = 0
	for n in fps_window:
		variance += pow(n - mean, 2)
	variance /= fps_window.size()

	frame_idx += 1

	display_text.text = (
		"Frame: %s\nFPS:\n    current: %.2f\n    average: %.2f\n    std dev: %.2f\n    low spike: %.2f"
		% [frame_idx, fps_current, mean, sqrt(variance), spike]
	)
