extends Node
## Autoload singleton ([code]GdPAIAutoload[/code]) that owns the addon's [GdPAIPlanScheduler].[br]
##[br]
## It creates the scheduler, applies the log level from [code]plugin.cfg[/code], and calls the
## scheduler's [code]process_callbacks[/code] every frame. That per-frame call is what runs the
## planning-time hooks ([method Action.get_action_cost], [method Action.simulate_effect], custom
## preconditions) on the main thread and delivers finished plans to their agents. Agents reach the
## scheduler through [method get_scheduler]; user code rarely needs to touch this class.

var _scheduler: GdPAIPlanScheduler


## Creates the scheduler, reads [code]plugin.cfg[/code] and applies the configured log level to
## the Rust engine, then clears debugger state for the new run.
func _ready() -> void:
	_scheduler = GdPAIPlanScheduler.new()
	_scheduler.name = "GdPAIPlanScheduler"
	add_child(_scheduler)
	_apply_log_level()
	if EngineDebugger.is_active():
		EngineDebugger.send_message("gdplanningai:clear_state", [])


## Pumps the scheduler once per frame: runs pending planning callbacks and delivers results.
func _process(_delta: float) -> void:
	if _scheduler:
		_scheduler.process_callbacks()


## Returns the background plan scheduler, or [code]null[/code] before the autoload is ready.
## Use it for diagnostics such as [code]set_log_level[/code] or [code]get_debug_tree(agent)[/code].
func get_scheduler() -> GdPAIPlanScheduler:
	return _scheduler


## Applies the [code]log_level[/code] from the [code]configuration[/code] section of
## [code]plugin.cfg[/code] (default 2, Info) to the scheduler.
func _apply_log_level() -> void:
	var config: ConfigFile = ConfigFile.new()
	var err: int = config.load("res://addons/GdPlanningAI/plugin.cfg")
	if err != OK:
		push_warning("GdPlanningAI: could not load plugin.cfg (error %d)" % err)
		return
	var level: int = config.get_value("configuration", "log_level", 2)
	_scheduler.set_log_level(level)
	print("GdPlanningAI: log level set to %d" % level)
