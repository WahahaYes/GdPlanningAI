//! Throwaway probe: a test that fails on purpose.

#[test]
fn gate_probe() {
    assert!(false, "gate probe: this test fails on purpose");
}
