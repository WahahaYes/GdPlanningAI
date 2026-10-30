//! Integration test for planner cancellation via cancel_flag.

mod common;
use common::{inputs, spawn_responder};

use gdplanningai_rust::plan_types::{
    ActionSpec, CallbackKind, CallbackResponse, GoalSpec, PlannerRunResult, PreconditionSpec,
};
use gdplanningai_rust::planner::{EngineConfig, EngineInputs, PlannerEngine};
use gdplanningai_rust::precondition::{PreconditionOp, PreconditionTarget};
use gdplanningai_rust::snapshot::{BlackboardSnapshot, VariantSnapshot};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn cancellation_returns_failure_quickly() {
    // Set up a planner with an action that has a custom validity check.
    // The custom check fires a callback that the responder answers slowly.
    // This parks the root node and keeps the search in Pending state,
    // giving us time to cancel.
    let agent = BlackboardSnapshot {
        properties: Arc::new(HashMap::new()),
        objects: Arc::new(HashMap::new()),
    };
    let world = BlackboardSnapshot {
        properties: Arc::new(HashMap::new()),
        objects: Arc::new(HashMap::new()),
    };

    let actions = vec![ActionSpec {
        name: "slow_action".to_string(),
        cost_callable_id: Some(1),
        effect_callable_id: Some(2),
        effect_spec: None,
        preconditions: vec![],
        validity_checks: vec![PreconditionSpec::Custom {
            callable_id: 99,
            dependent_object_ids: vec![],
        }],
        requirements: vec![],
        provisions: vec![],
        dependent_object_ids: vec![],
    }];

    let goals = vec![GoalSpec {
        name: "impossible".to_string(),
        reward: 10.0,
        desired_state: vec![PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::Equal,
            property_name: "magic".to_string(),
            value: Some(VariantSnapshot::Bool(true)),
        }],
        original_index: 0,
    }];

    // Spawn a slow responder so the callback stays pending for a while.
    let req_tx = spawn_responder(|req| {
        thread::sleep(Duration::from_millis(100));
        match req.kind {
            CallbackKind::GetCost { .. } => CallbackResponse::Float(1.0),
            CallbackKind::ApplyEffect { mut agent, world } => {
                Arc::make_mut(&mut Arc::make_mut(&mut agent).properties)
                    .insert("magic".to_string(), VariantSnapshot::Bool(true));
                CallbackResponse::UpdatedSnapshots(agent, world)
            }
            CallbackKind::EvalCustomPrecond { .. } => CallbackResponse::Bool(true),
        }
    });

    let cancel_flag = Arc::new(AtomicBool::new(false));
    let mut engine = PlannerEngine::new(
        EngineInputs {
            cancel_flag: cancel_flag.clone(),
            ..inputs(actions, agent, world, req_tx)
        },
        EngineConfig {
            max_depth: 10,
            ..EngineConfig::default()
        },
    );

    let start_time = Instant::now();
    let mut iterations = 0;
    let mut saw_pending = false;
    loop {
        if iterations >= 3 {
            cancel_flag.store(true, Ordering::Relaxed);
        }

        match engine.plan(&goals) {
            PlannerRunResult::Complete(res) => {
                if cancel_flag.load(Ordering::Relaxed) {
                    assert!(
                        res.is_none(),
                        "Planner should have returned None after cancellation"
                    );
                    break;
                }
                // If we complete before cancelling, something went wrong
                panic!("Planner completed before cancellation could take effect");
            }
            PlannerRunResult::Pending(_) => {
                saw_pending = true;
                iterations += 1;
                thread::sleep(Duration::from_millis(5));
            }
        }
    }

    let elapsed = start_time.elapsed();
    assert!(saw_pending, "Planner should have entered Pending state");
    assert!(
        elapsed < Duration::from_secs(2),
        "Cancellation took too long: {:?}",
        elapsed
    );
}
