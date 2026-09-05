extends GutTest


func test_plugin_cfg_default_log_level_is_info():
	var config = ConfigFile.new()
	var err = config.load("res://addons/GdPlanningAI/plugin.cfg")
	assert_eq(err, OK, "plugin.cfg should load")
	assert_eq(
		config.get_value("configuration", "log_level", -1),
		2,
		"Default log level should be Info (2); Debug/Trace are opt-in"
	)
