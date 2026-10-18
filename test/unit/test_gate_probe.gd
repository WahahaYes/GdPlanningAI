extends GutTest


## Throwaway probe: a test that fails on purpose.
func test_gate_probe() -> void:
	assert_true(false, "gate probe: this test fails on purpose")
