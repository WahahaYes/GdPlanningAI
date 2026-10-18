//! Finished planner engines are freed off the calling (main) thread.

use gdplanningai_rust::scheduler::drop_in_background;
use std::sync::mpsc::{Sender, channel};
use std::thread::{self, ThreadId};

/// Reports the thread it is dropped on.
struct DropProbe(Sender<ThreadId>);

impl Drop for DropProbe {
    fn drop(&mut self) {
        let _ = self.0.send(thread::current().id());
    }
}

#[test]
fn drops_items_on_another_thread() {
    let (tx, rx) = channel();
    let handle = drop_in_background(vec![DropProbe(tx.clone()), DropProbe(tx)])
        .expect("non-empty input spawns a thread");
    handle.join().unwrap();
    let caller = thread::current().id();
    let dropped_on: Vec<ThreadId> = rx.try_iter().collect();
    assert_eq!(dropped_on.len(), 2, "every item is dropped");
    assert!(
        dropped_on.iter().all(|id| *id != caller),
        "never on the caller's thread"
    );
}

#[test]
fn empty_input_spawns_nothing() {
    assert!(drop_in_background(Vec::<u8>::new()).is_none());
}
