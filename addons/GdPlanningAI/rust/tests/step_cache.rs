//! Verification shares simulated steps: branches replaying the same action from the same state
//! wait on one effect request, later ones reuse its answer, and all hold the same post-state.

mod common;
use common::{create_test_agent, create_test_world, inputs};
use gdplanningai_rust::plan_types::{ActionSpec, CallbackKind, CallbackRequest, CallbackResponse};
use gdplanningai_rust::planner::simulation::StepResult;
use gdplanningai_rust::planner::types::{BranchState, PlanBranch};
use gdplanningai_rust::planner::{EngineConfig, PlannerEngine};
use gdplanningai_rust::snapshot::VariantSnapshot;
use std::sync::{Arc, mpsc};

/// An engine with one effect-only action; the returned receiver sees every callback request.
fn engine_with_effect_action() -> (PlannerEngine, mpsc::Receiver<CallbackRequest>) {
    let (req_tx, req_rx) = mpsc::channel::<CallbackRequest>();
    let actions = vec![ActionSpec {
        name: "eat".to_string(),
        cost_callable_id: None,
        effect_callable_id: Some(0),
        preconditions: vec![],
        validity_checks: vec![],
        requirements: vec![],
        provisions: vec![],
        dependent_object_ids: vec![],
    }];
    let engine = PlannerEngine::new(
        inputs(
            actions,
            create_test_agent(vec![("hunger", VariantSnapshot::Int(80))]),
            create_test_world(vec![], vec![]),
            req_tx,
        ),
        EngineConfig {
            max_depth: 4,
            ..EngineConfig::default()
        },
    );
    (engine, req_rx)
}

/// A verifying one-action chain whose cost is already known, as after discovery.
fn verifying_branch(engine: &PlannerEngine) -> PlanBranch {
    let mut branch = PlanBranch::new(
        Arc::clone(&engine.ctx.initial_agent),
        Arc::clone(&engine.ctx.initial_world),
    );
    branch.state = BranchState::Verifying;
    branch.action_chain = vec![0];
    branch.action_costs = vec![1.0];
    branch
}

#[test]
fn branches_replaying_the_same_step_share_one_request_and_its_state() {
    let (mut engine, requests) = engine_with_effect_action();
    let mut a = verifying_branch(&engine);
    let mut b = verifying_branch(&engine);

    let StepResult::Pending(id_a) = engine.simulate_and_advance(&mut a, 0, &[], None) else {
        panic!("first branch should request the effect");
    };
    let StepResult::Pending(id_b) = engine.simulate_and_advance(&mut b, 0, &[], None) else {
        panic!("second branch should wait");
    };
    assert_eq!(
        id_a, id_b,
        "the second branch waits on the first one's request"
    );
    let request = requests.try_recv().expect("one effect request");
    assert!(matches!(request.kind, CallbackKind::ApplyEffect { .. }));
    assert!(requests.try_recv().is_err(), "no second request");

    let mut agent = (*engine.ctx.initial_agent).clone();
    agent
        .properties
        .insert("hunger".to_string(), VariantSnapshot::Int(20));
    let answer = CallbackResponse::UpdatedSnapshots(agent, (*engine.ctx.initial_world).clone());
    for branch in [&mut a, &mut b] {
        assert!(matches!(
            engine.simulate_and_advance(branch, 0, &[], Some(&answer)),
            StepResult::Ready(())
        ));
    }
    assert!(Arc::ptr_eq(&a.current_agent, &b.current_agent));
    assert!(Arc::ptr_eq(&a.current_world, &b.current_world));
    assert_eq!(
        a.current_agent.properties.get("hunger"),
        Some(&VariantSnapshot::Int(20))
    );

    // A branch reaching the step later reuses the post-state without a callback.
    let mut c = verifying_branch(&engine);
    assert!(matches!(
        engine.simulate_and_advance(&mut c, 0, &[], None),
        StepResult::Ready(())
    ));
    assert!(Arc::ptr_eq(&a.current_agent, &c.current_agent));
    assert!(requests.try_recv().is_err(), "no request for a cached step");
}
