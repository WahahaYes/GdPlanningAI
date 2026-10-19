extends GutTest


## The Rust library reports the version the addon's plugin.cfg declares, so a stale binary shows.
func test_engine_version_matches_plugin_cfg() -> void:
	var config: ConfigFile = ConfigFile.new()
	assert_eq(config.load("res://addons/GdPlanningAI/plugin.cfg"), OK, "plugin.cfg should load")
	var scheduler: GdPAIPlanScheduler = GdPAIPlanScheduler.new()
	assert_eq(
		scheduler.get_engine_version(),
		config.get_value("plugin", "version", ""),
		"Rebuild the Rust library (make -C addons/GdPlanningAI/rust build-release)"
	)
	scheduler.free()
