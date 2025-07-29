# Simple FPS counter for monitoring the performance in GdPAI demos.

extends Node

## Reference to a label for displaying fps results.
@export var display_text: Label
## How long of a window to store FPS data.
@export var window_size: int = 180
## Array to store fps values.
@onready var fps_window: Array[float] = []


func _process(delta: float) -> void:
	fps_window.append(1 / delta)
	if fps_window.size() > window_size:
		fps_window.pop_front()

	var sum: float = 0
	for n in fps_window:
		sum += float(n)  # Ensure n is treated as a float for accurate division
	var mean: float = sum / fps_window.size()
	var spike: float = fps_window.min()

	display_text.text = "FPS average: %.2f\nLow spike: %.2f" % [mean, spike]
