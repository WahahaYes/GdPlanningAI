//! Planner scenarios as data: runs every case in `tests/scenarios/*.toml` through the engine's
//! entry point, with a scripted host answering costs, effects and custom preconditions from the
//! fixture. Each file is named after the `docs/BEHAVIORS.md` slug it proves; a case reports as
//! `<slug>::<name>`. The format is described in `tests/scenarios/README.md`.

mod common;
use common::{create_sim_object, run_to_completion, spawn_responder};

use gdplanningai_rust::plan_types::{
    ActionSpec, CallbackKind, CallbackRequest, CallbackResponse, GoalSpec, PreconditionSpec,
};
use gdplanningai_rust::planner::{EngineConfig, EngineInputs, PlannerEngine, TerminationStrategy};
use gdplanningai_rust::precondition::{PreconditionOp, PreconditionTarget};
use gdplanningai_rust::requirement::{ProvisionSpec, RequirementSpec};
use gdplanningai_rust::snapshot::{BlackboardSnapshot, VariantSnapshot};
use libtest_mimic::{Arguments, Failed, Trial};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    case: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    max_depth: Option<usize>,
    #[serde(default)]
    first_complete: bool,
    #[serde(default)]
    agent: toml::Table,
    #[serde(default)]
    agent_objects: Vec<Object>,
    #[serde(default)]
    world: toml::Table,
    #[serde(default)]
    objects: Vec<Object>,
    #[serde(default)]
    provisions: Vec<Provision>,
    #[serde(default)]
    actions: Vec<Action>,
    goals: Vec<Goal>,
    expect: Expect,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Object {
    uid: String,
    #[serde(default)]
    groups: Vec<String>,
    #[serde(default)]
    properties: toml::Table,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Action {
    name: String,
    cost: Option<f64>,
    effect: Option<Effect>,
    #[serde(default)]
    preconditions: Vec<Condition>,
    #[serde(default)]
    validity: Vec<Condition>,
    #[serde(default)]
    requires: Vec<Requirement>,
    #[serde(default)]
    provides: Vec<Provision>,
}

/// Agent properties an effect writes (`set`) or adds to (`add`, ints).
#[derive(Deserialize, Clone, Default)]
#[serde(deny_unknown_fields)]
struct Effect {
    #[serde(default)]
    set: toml::Table,
    #[serde(default)]
    add: toml::Table,
}

/// A built-in precondition on one of `agent`, `world` or a `group`'s `prop`, or a `custom` one
/// the host answers.
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct Condition {
    agent: Option<String>,
    world: Option<String>,
    group: Option<String>,
    prop: Option<String>,
    #[serde(default = "has_op")]
    op: String,
    value: Option<toml::Value>,
    custom: Option<Box<Custom>>,
}

fn has_op() -> String {
    "has".to_string()
}

/// How the host answers a custom precondition: a constant, whether a binding the planner injected
/// holds `value`, or a built-in condition on the state the callback receives.
#[derive(Deserialize, Clone)]
#[serde(untagged)]
enum Custom {
    Always(bool),
    Binding { binding: String, value: toml::Value },
    State(Condition),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Requirement {
    binding_exists: Option<String>,
    binding_equals: Option<String>,
    binding_in_set: Option<String>,
    set: Option<String>,
    fact: Option<String>,
    value: Option<toml::Value>,
    #[serde(default)]
    args: Vec<toml::Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Provision {
    binding: Option<String>,
    fact: Option<String>,
    fact_wildcard: Option<String>,
    value: Option<toml::Value>,
    #[serde(default)]
    args: Vec<toml::Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Goal {
    name: String,
    #[serde(default = "default_reward")]
    reward: f64,
    want: Vec<Condition>,
}

fn default_reward() -> f64 {
    1.0
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expect {
    #[serde(default)]
    fails: bool,
    plan: Option<Vec<String>>,
    cost: Option<f64>,
    goal: Option<String>,
    #[serde(default)]
    bindings: Vec<ExpectedBinding>,
}

/// A binding the plan must contain at chain position `at`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedBinding {
    at: i64,
    name: String,
    values: Vec<toml::Value>,
}

/// What the host does for one callable id.
enum Answer {
    Cost(f64),
    Effect(Effect),
    Custom(Custom),
}

fn value(v: &toml::Value) -> Result<VariantSnapshot, String> {
    Ok(match v {
        toml::Value::Integer(i) => VariantSnapshot::Int(*i),
        toml::Value::Float(f) => VariantSnapshot::Float(f.to_bits()),
        toml::Value::Boolean(b) => VariantSnapshot::Bool(*b),
        toml::Value::String(s) => VariantSnapshot::Str(s.clone()),
        toml::Value::Table(t) => match t.get("object") {
            Some(toml::Value::Integer(id)) if t.len() == 1 => VariantSnapshot::ObjectRef(*id),
            _ => return Err(format!("unsupported value {v}: use {{ object = <id> }}")),
        },
        _ => return Err(format!("unsupported value {v}")),
    })
}

fn values(vs: &[toml::Value]) -> Result<Vec<VariantSnapshot>, String> {
    vs.iter().map(value).collect()
}

fn snapshot(props: &toml::Table, objects: &[Object]) -> Result<BlackboardSnapshot, String> {
    let mut snap = BlackboardSnapshot {
        properties: HashMap::new(),
        objects: HashMap::new(),
    };
    for (k, v) in props {
        snap.properties.insert(k.clone(), value(v)?);
    }
    for obj in objects {
        let mut data = create_sim_object(
            &obj.uid,
            obj.groups.iter().map(String::as_str).collect(),
            vec![],
        );
        for (k, v) in &obj.properties {
            data.properties.insert(k.clone(), value(v)?);
        }
        snap.objects.insert(obj.uid.clone(), data);
    }
    Ok(snap)
}

fn op(name: &str) -> Result<PreconditionOp, String> {
    Ok(match name {
        "has" => PreconditionOp::HasProperty,
        "==" => PreconditionOp::Equal,
        "!=" => PreconditionOp::NotEqual,
        ">" => PreconditionOp::GreaterThan,
        ">=" => PreconditionOp::GreaterThanOrEqual,
        "<" => PreconditionOp::LessThan,
        "<=" => PreconditionOp::LessThanOrEqual,
        _ => return Err(format!("unknown op {name:?}")),
    })
}

/// A built-in spec for `c`, ignoring `custom`.
fn builtin(c: &Condition) -> Result<PreconditionSpec, String> {
    let (target, property_name) = match (&c.agent, &c.world, &c.group) {
        (Some(p), None, None) => (PreconditionTarget::Agent, p.clone()),
        (None, Some(p), None) => (PreconditionTarget::WorldState, p.clone()),
        (None, None, Some(g)) => {
            let property = c.prop.clone().ok_or("a group condition needs prop")?;
            (
                PreconditionTarget::WorldObjectProxy {
                    group: g.clone(),
                    property: property.clone(),
                },
                property,
            )
        }
        _ => return Err("a condition names exactly one of agent, world, group".to_string()),
    };
    Ok(PreconditionSpec::Builtin {
        target,
        operation: op(&c.op)?,
        property_name,
        value: c.value.as_ref().map(value).transpose()?,
    })
}

/// Builds the spec for `c`, registering custom conditions as host answers.
fn condition(c: &Condition, answers: &mut Vec<Answer>) -> Result<PreconditionSpec, String> {
    match &c.custom {
        Some(custom) => {
            answers.push(Answer::Custom((**custom).clone()));
            Ok(PreconditionSpec::Custom {
                callable_id: answers.len() - 1,
                dependent_object_ids: vec![],
            })
        }
        None => builtin(c),
    }
}

fn conditions(
    cs: &[Condition],
    answers: &mut Vec<Answer>,
) -> Result<Vec<PreconditionSpec>, String> {
    cs.iter().map(|c| condition(c, answers)).collect()
}

fn requirement(r: &Requirement) -> Result<RequirementSpec, String> {
    Ok(
        match (
            &r.binding_exists,
            &r.binding_equals,
            &r.binding_in_set,
            &r.fact,
        ) {
            (Some(b), None, None, None) => RequirementSpec::BindingExists {
                binding_name: b.clone(),
            },
            (None, Some(b), None, None) => RequirementSpec::BindingEquals {
                binding_name: b.clone(),
                value: value(r.value.as_ref().ok_or("binding_equals needs value")?)?,
            },
            (None, None, Some(b), None) => RequirementSpec::BindingInSet {
                binding_name: b.clone(),
                set_name: r.set.clone().ok_or("binding_in_set needs set")?,
            },
            (None, None, None, Some(f)) => RequirementSpec::Fact {
                fact_name: f.clone(),
                args: values(&r.args)?,
            },
            _ => return Err("a requirement has exactly one kind".to_string()),
        },
    )
}

fn provision(p: &Provision) -> Result<ProvisionSpec, String> {
    Ok(match (&p.binding, &p.fact, &p.fact_wildcard) {
        (Some(b), None, None) => ProvisionSpec::Binding {
            binding_name: b.clone(),
            value: value(p.value.as_ref().ok_or("binding needs value")?)?,
        },
        (None, Some(f), None) => ProvisionSpec::Fact {
            fact_name: f.clone(),
            args: values(&p.args)?,
        },
        (None, None, Some(f)) => ProvisionSpec::FactWildcard {
            fact_name: f.clone(),
        },
        _ => return Err("a provision has exactly one kind".to_string()),
    })
}

fn apply(effect: &Effect, agent: &mut BlackboardSnapshot) {
    for (k, v) in &effect.set {
        agent
            .properties
            .insert(k.clone(), value(v).expect("checked at load"));
    }
    for (k, v) in &effect.add {
        let delta = v.as_integer().expect("checked at load");
        let current = match agent.properties.get(k) {
            Some(VariantSnapshot::Int(i)) => *i,
            _ => 0,
        };
        agent
            .properties
            .insert(k.clone(), VariantSnapshot::Int(current + delta));
    }
}

fn answer_custom(
    custom: &Custom,
    req: &CallbackRequest,
    agent: &BlackboardSnapshot,
    world: &BlackboardSnapshot,
) -> bool {
    match custom {
        Custom::Always(b) => *b,
        Custom::Binding { binding, value: v } => {
            let want = value(v).expect("checked at load");
            req.bindings
                .iter()
                .any(|(name, vals)| name == binding && vals.contains(&want))
        }
        Custom::State(c) => builtin(c)
            .expect("checked at load")
            .evaluate_builtin(agent, world)
            .unwrap_or(false),
    }
}

/// The scripted host: answers each request from the fixture's registered answers.
fn host(answers: Vec<Answer>) -> std::sync::mpsc::Sender<CallbackRequest> {
    let answers = Arc::new(answers);
    spawn_responder(move |req| {
        let answer = &answers[req.callable_id];
        match (&req.kind, answer) {
            (CallbackKind::GetCost { .. }, Answer::Cost(c)) => CallbackResponse::Float(*c),
            (CallbackKind::ApplyEffect { agent, world }, Answer::Effect(e)) => {
                let mut agent = agent.clone();
                apply(e, &mut agent);
                CallbackResponse::UpdatedSnapshots(agent, world.clone())
            }
            (CallbackKind::EvalCustomPrecond { agent, world }, Answer::Custom(c)) => {
                CallbackResponse::Bool(answer_custom(c, &req, agent, world))
            }
            _ => panic!("callable {} called for the wrong kind", req.callable_id),
        }
    })
}

/// Checks the fixture values `run` would otherwise only meet on the host thread.
fn validate_effect(e: &Effect) -> Result<(), String> {
    for v in e.set.values() {
        value(v)?;
    }
    for (k, v) in &e.add {
        v.as_integer()
            .ok_or_else(|| format!("add.{k} must be an integer"))?;
    }
    Ok(())
}

fn validate_custom(c: &Custom) -> Result<(), String> {
    match c {
        Custom::Always(_) => Ok(()),
        Custom::Binding { value: v, .. } => value(v).map(|_| ()),
        Custom::State(cond) => builtin(cond).map(|_| ()),
    }
}

fn run(case: &Case) -> Result<(), String> {
    let mut answers = Vec::new();
    let mut actions = Vec::new();
    for a in &case.actions {
        let cost_callable_id = a.cost.map(|c| {
            answers.push(Answer::Cost(c));
            answers.len() - 1
        });
        let effect_callable_id = match &a.effect {
            Some(e) => {
                validate_effect(e)?;
                answers.push(Answer::Effect(e.clone()));
                Some(answers.len() - 1)
            }
            None => None,
        };
        actions.push(ActionSpec {
            name: a.name.clone(),
            cost_callable_id,
            effect_callable_id,
            preconditions: conditions(&a.preconditions, &mut answers)?,
            validity_checks: conditions(&a.validity, &mut answers)?,
            requirements: a
                .requires
                .iter()
                .map(requirement)
                .collect::<Result<_, _>>()?,
            provisions: a.provides.iter().map(provision).collect::<Result<_, _>>()?,
            dependent_object_ids: vec![],
        });
    }
    let goals = case
        .goals
        .iter()
        .enumerate()
        .map(|(i, g)| {
            Ok(GoalSpec {
                name: g.name.clone(),
                reward: g.reward,
                desired_state: conditions(&g.want, &mut answers)?,
                original_index: i,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    for a in &answers {
        if let Answer::Custom(c) = a {
            validate_custom(c)?;
        }
    }

    let mut config = EngineConfig::default();
    if let Some(depth) = case.max_depth {
        config.max_depth = depth;
    }
    if case.first_complete {
        config.termination = TerminationStrategy::FirstComplete;
    }
    let mut engine = PlannerEngine::new(
        EngineInputs {
            actions,
            agent: snapshot(&case.agent, &case.agent_objects)?,
            world: snapshot(&case.world, &case.objects)?,
            initial_provisions: case
                .provisions
                .iter()
                .map(provision)
                .collect::<Result<_, _>>()?,
            request_tx: host(answers),
            cancel_flag: Arc::default(),
        },
        config,
    );
    let plan = run_to_completion(&mut engine, &goals).ok_or("search was cancelled")?;

    let expect = &case.expect;
    if expect.fails {
        return if plan.success {
            Err(format!(
                "expected failure, got plan {:?}",
                names(case, &plan.action_chain)
            ))
        } else {
            Ok(())
        };
    }
    if !plan.success {
        return Err("expected a plan, the search failed".to_string());
    }
    if let Some(want) = &expect.plan {
        let got = names(case, &plan.action_chain);
        if &got != want {
            return Err(format!("plan {got:?}, expected {want:?}"));
        }
    }
    if let Some(cost) = expect.cost
        && (plan.total_cost - cost).abs() > 1e-9
    {
        return Err(format!("cost {}, expected {cost}", plan.total_cost));
    }
    if let Some(goal) = &expect.goal {
        let got = &case.goals[plan.goal_index as usize].name;
        if got != goal {
            return Err(format!("goal {got:?}, expected {goal:?}"));
        }
    }
    for b in &expect.bindings {
        let want = (b.at, b.name.clone(), values(&b.values)?);
        if !plan.action_bindings.contains(&want) {
            return Err(format!(
                "missing binding {want:?}, got {:?}",
                plan.action_bindings
            ));
        }
    }
    Ok(())
}

fn names(case: &Case, chain: &[i64]) -> Vec<String> {
    chain
        .iter()
        .map(|&i| case.actions[i as usize].name.clone())
        .collect()
}

fn trials() -> Vec<Trial> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios");
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .expect("tests/scenarios exists")
        .map(|e| e.expect("readable entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "toml"))
        .collect();
    paths.sort();
    let mut trials = Vec::new();
    for path in paths {
        let slug = path.file_stem().unwrap().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(&path).expect("readable fixture");
        let fixture: Fixture = match toml::from_str(&text) {
            Ok(f) => f,
            Err(e) => {
                let msg = format!("{}: {e}", path.display());
                trials.push(Trial::test(format!("{slug}::parse"), move || {
                    Err(msg.into())
                }));
                continue;
            }
        };
        for case in fixture.case {
            let name = format!("{slug}::{}", case.name);
            trials.push(Trial::test(name, move || run(&case).map_err(Failed::from)));
        }
    }
    trials
}

fn main() {
    gdplanningai_rust::logger::init_log_channel();
    gdplanningai_rust::logger::set_log_level(gdplanningai_rust::logger::LogLevel::Debug);
    libtest_mimic::run(&Arguments::from_args(), trials()).exit();
}
