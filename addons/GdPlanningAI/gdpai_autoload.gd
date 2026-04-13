extends Node
## This is an autoload node for easy reference to the GdPlanningAI addon.
## It initialises the background plan scheduler and forwards frame ticks
## to it so that callback requests from background threads are processed.

var _scheduler: GdPAIPlanScheduler


## Reads plugin.cfg and applies the configured log level to the Rust engine,
## then clears debugger state for the new run.
func _ready() -> void:
	_apply_log_level()
	EngineDebugger.send_message("gdplanningai:clear_state", [])
	_scheduler = GdPAIPlanScheduler.new()
	_scheduler.name = "GdPAIPlanScheduler"
	add_child(_scheduler)


func _process(_delta: float) -> void:
	if _scheduler:
		_scheduler.process_callbacks()


## Returns the background plan scheduler instance.
func get_scheduler() -> GdPAIPlanScheduler:
	return _scheduler


func _apply_log_level() -> void:
	var config := ConfigFile.new()
	var err := config.load("res://addons/GdPlanningAI/plugin.cfg")
	if err != OK:
		push_warning("GdPlanningAI: could not load plugin.cfg (error %d)" % err)
		return
	var level: int = config.get_value("configuration", "log_level", 2)
	var engine: RustPlanningEngine = RustPlanningEngine.new()
	engine.set_log_level(level)
	print("GdPlanningAI: log level set to %d" % level)
