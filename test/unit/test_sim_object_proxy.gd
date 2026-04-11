extends GutTest

func test_proxy_property_operations():
	var proxy = SimObjectProxy.new()
	
	proxy.set_property("health", 100)
	assert_eq(proxy.get_property("health"), 100, "Should retrieve set property")
	assert_true(proxy.has_property("health"), "Should have property after setting")

func test_proxy_property_types():
	var proxy = SimObjectProxy.new()
	
	proxy.set_property("int_val", 42)
	proxy.set_property("float_val", 3.14)
	proxy.set_property("string_val", "test")
	proxy.set_property("bool_val", true)
	
	assert_eq(proxy.get_property("int_val"), 42)
	assert_eq(proxy.get_property("float_val"), 3.14)
	assert_eq(proxy.get_property("string_val"), "test")
	assert_eq(proxy.get_property("bool_val"), true)

func test_proxy_missing_property():
	var proxy = SimObjectProxy.new()
	
	assert_eq(proxy.get_property("nonexistent"), null, "Missing property returns null")
	assert_false(proxy.has_property("nonexistent"), "Missing property has_property false")

func test_proxy_property_overwrite():
	var proxy = SimObjectProxy.new()
	
	proxy.set_property("value", 10)
	proxy.set_property("value", 20)
	
	assert_eq(proxy.get_property("value"), 20, "Should overwrite existing property")
