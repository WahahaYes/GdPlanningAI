//! Native effect specs: declarative action effects evaluated on snapshots.
//!
//! An action may declare `effect_spec` (see `Action::get_effect_spec`) alongside its
//! `simulate_effect` callable. When every entry parses, the planner applies the spec itself
//! ([`apply_effect_spec`]) and never round-trips to GDScript; otherwise it falls back to the
//! callable. Six ops cover the built-in actions: `set`, `inc`, `dec`, `min_cap`, `max_floor`
//! and `copy`. Lookup amounts model table-driven costs like hunger restored per food item.

use crate::snapshot::{BlackboardSnapshot, SimObjectData, VariantSnapshot};
use godot::prelude::*;
use std::collections::HashMap;

/// Where a value-writing op stores its result.
#[derive(Clone, Debug, PartialEq)]
pub enum EffectTarget {
    /// An agent blackboard property.
    AgentProp {
        /// Property name; created when missing, like `set_property`.
        prop: String,
    },
    /// A property of the world object with this uid; missing objects skip the op.
    ObjectProp {
        /// Object uid (instance id as a string).
        uid: String,
        /// Property name; created when missing.
        prop: String,
    },
}

/// Where a `Copy` reads its value from.
#[derive(Clone, Debug, PartialEq)]
pub enum CopySource {
    /// A property of the world object with this uid.
    ObjectProp {
        /// Object uid (instance id as a string).
        uid: String,
        /// Property to read; a missing key skips the op.
        prop: String,
    },
    /// A property of the world object the binding resolves to.
    BindingProp {
        /// Binding name; unbound or unresolvable bindings skip the op.
        binding: String,
        /// Property to read from the resolved object.
        prop: String,
    },
}

/// Where a `Copy` writes its value to.
#[derive(Clone, Debug, PartialEq)]
pub enum CopyDest {
    /// An agent blackboard property.
    AgentProp {
        /// Property name; created when missing.
        prop: String,
    },
    /// A property of the agent's object in this group; missing proxies skip the op.
    AgentGroupProp {
        /// Group the agent object must belong to.
        group: String,
        /// Property name; created when missing.
        prop: String,
    },
    /// A property of the world object with this uid; missing objects skip the op.
    ObjectProp {
        /// Object uid (instance id as a string).
        uid: String,
        /// Property name; created when missing.
        prop: String,
    },
}

/// The amount a `Dec` subtracts.
#[derive(Clone, Debug, PartialEq)]
pub enum EffectAmount {
    /// A constant.
    Direct(f64),
    /// Looked up from the agent property or binding at runtime.
    Lookup {
        /// Binding (or agent property) holding the table key.
        binding: String,
        /// Key to amount; a bound-but-absent key aborts the remaining ops.
        table: HashMap<String, f64>,
        /// Amount used when nothing is bound yet (optimistic discovery).
        default: f64,
    },
}

/// One declarative effect step; a spec is a list applied in order.
#[derive(Clone, Debug, PartialEq)]
pub enum EffectOp {
    /// Set the target to a Tier-1 value.
    Set {
        /// Where to store the value.
        target: EffectTarget,
        /// Bool, int, finite float or string.
        value: VariantSnapshot,
    },
    /// Add the amount to the target's number; non-numeric targets skip the op.
    Inc {
        /// Where to add.
        target: EffectTarget,
        /// Finite amount to add.
        amount: f64,
    },
    /// Subtract the amount from the target's number; non-numeric targets skip the op.
    Dec {
        /// Where to subtract.
        target: EffectTarget,
        /// How much to subtract.
        amount: EffectAmount,
    },
    /// Clamp the target down to at most the cap; non-numeric targets skip the op.
    MinCap {
        /// Where to clamp.
        target: EffectTarget,
        /// Finite upper bound.
        cap: f64,
    },
    /// Clamp the target up to at least the floor; non-numeric targets skip the op.
    MaxFloor {
        /// Where to clamp.
        target: EffectTarget,
        /// Finite lower bound.
        floor: f64,
    },
    /// Copy one property to another, preserving whatever tier it holds.
    Copy {
        /// Where to read from.
        from: CopySource,
        /// Where to write to.
        to: CopyDest,
    },
}

/// Applies `ops` to clones of the snapshots and returns the post-state.
///
/// The three property maps are cloned once; the world top-level properties are never written.
/// Bindings are injected into the agent clone first (mirroring the callback path), then each
/// op runs in order. A `Lookup` miss aborts the remaining ops but keeps earlier ones.
pub fn apply_effect_spec(
    ops: &[EffectOp],
    agent: &BlackboardSnapshot,
    world: &BlackboardSnapshot,
    bindings: &[(String, Vec<VariantSnapshot>)],
) -> (BlackboardSnapshot, BlackboardSnapshot) {
    let mut agent_props = agent.properties.clone();
    let mut agent_objects = agent.objects.clone();
    let mut world_objects = world.objects.clone();
    for (name, values) in bindings {
        if !name.is_empty() && !values.is_empty() {
            agent_props.insert(name.clone(), values[0].clone());
        }
    }
    for op in ops {
        if !op.apply_to(&mut agent_props, &mut agent_objects, &mut world_objects) {
            break;
        }
    }
    (
        BlackboardSnapshot {
            properties: agent_props,
            objects: agent_objects,
        },
        BlackboardSnapshot {
            properties: world.properties.clone(),
            objects: world_objects,
        },
    )
}

/// Non-empty string, or `None` when missing or empty.
fn non_empty(value: Option<&str>) -> Option<&str> {
    value.filter(|s| !s.is_empty())
}

/// Required non-empty string field of a spec dictionary.
fn req_string(dict: &VarDictionary, key: &str) -> Option<String> {
    let value: String = dict.get(key)?.try_to().ok()?;
    if value.is_empty() { None } else { Some(value) }
}

/// A finite number, accepting int and float variants.
fn variant_f64(value: &Variant) -> Option<f64> {
    if let Ok(float) = value.try_to::<f64>() {
        Some(float)
    } else if let Ok(int) = value.try_to::<i64>() {
        Some(int as f64)
    } else {
        None
    }
}

/// A finite number, or `None` for missing, non-numeric or non-finite input.
fn finite(value: Option<f64>) -> Option<f64> {
    value.filter(|f| f.is_finite())
}

/// A Tier-1 snapshot (bool, int, finite float, string); the only values `Set` accepts.
fn tier1(value: &VariantSnapshot) -> Option<VariantSnapshot> {
    match value {
        VariantSnapshot::Bool(_) | VariantSnapshot::Int(_) | VariantSnapshot::Str(_) => {
            Some(value.clone())
        }
        VariantSnapshot::Float(bits) if f64::from_bits(*bits).is_finite() => Some(value.clone()),
        _ => None,
    }
}

impl EffectTarget {
    /// Parses `{"target": kind, ...}`: `agent` needs `prop`, `object` needs `uid` and `prop`.
    fn parse(kind: &str, uid: Option<&str>, prop: Option<&str>) -> Option<Self> {
        let prop = non_empty(prop)?.to_string();
        match kind {
            "agent" => Some(Self::AgentProp { prop }),
            "object" => Some(Self::ObjectProp {
                uid: non_empty(uid)?.to_string(),
                prop,
            }),
            _ => None,
        }
    }

    /// Extracts the target of a value-writing op from its dictionary.
    fn from_op_dict(dict: &VarDictionary) -> Option<Self> {
        let kind: String = dict.get("target")?.try_to().ok()?;
        let uid: Option<String> = dict.get("uid").and_then(|v| v.try_to().ok());
        let prop: Option<String> = dict.get("prop").and_then(|v| v.try_to().ok());
        Self::parse(&kind, uid.as_deref(), prop.as_deref())
    }

    /// Mutable slot for the target without creating anything. Missing agents, objects or
    /// properties yield `None` and skip the op.
    fn resolve_existing<'a>(
        &self,
        agent_props: &'a mut HashMap<String, VariantSnapshot>,
        world_objects: &'a mut HashMap<String, SimObjectData>,
    ) -> Option<&'a mut VariantSnapshot> {
        match self {
            Self::AgentProp { prop } => agent_props.get_mut(prop),
            Self::ObjectProp { uid, prop } => world_objects.get_mut(uid)?.properties.get_mut(prop),
        }
    }

    /// Mutable slot for the target, creating agent/object properties like `set_property`.
    /// Missing objects yield `None` and skip the op.
    fn resolve_or_create<'a>(
        &self,
        agent_props: &'a mut HashMap<String, VariantSnapshot>,
        world_objects: &'a mut HashMap<String, SimObjectData>,
    ) -> Option<&'a mut VariantSnapshot> {
        match self {
            Self::AgentProp { prop } => Some(
                agent_props
                    .entry(prop.clone())
                    .or_insert(VariantSnapshot::Nil),
            ),
            Self::ObjectProp { uid, prop } => world_objects.get_mut(uid).map(|obj| {
                obj.properties
                    .entry(prop.clone())
                    .or_insert(VariantSnapshot::Nil)
            }),
        }
    }
}

impl CopySource {
    /// Parses the `from` half of a copy: `{"object": uid, "prop": p}` or the binding form.
    fn parse(dict: &VarDictionary) -> Option<Self> {
        let prop = req_string(dict, "prop")?;
        let uid: Option<String> = dict.get("object").and_then(|v| v.try_to().ok());
        if let Some(uid) = uid.filter(|s| !s.is_empty()) {
            return Some(Self::ObjectProp { uid, prop });
        }
        let binding: String = dict.get("binding").and_then(|v| v.try_to().ok())?;
        if binding.is_empty() {
            return None;
        }
        Some(Self::BindingProp { binding, prop })
    }

    /// Reads the source value; unresolvable sources yield `None` and skip the op.
    fn read(
        &self,
        agent_props: &HashMap<String, VariantSnapshot>,
        world_objects: &HashMap<String, SimObjectData>,
    ) -> Option<VariantSnapshot> {
        match self {
            Self::ObjectProp { uid, prop } => world_objects.get(uid)?.properties.get(prop).cloned(),
            Self::BindingProp { binding, prop } => {
                let key = agent_props.get(binding)?;
                resolve_object(key, world_objects)?
                    .properties
                    .get(prop)
                    .cloned()
            }
        }
    }
}

impl CopyDest {
    /// Parses the `to` half of a copy: `{"agent": prop}`, the agent-group form with
    /// `{"agent_group": group, "prop": p}`, or the object form with `uid` and `prop`.
    fn parse(dict: &VarDictionary) -> Option<Self> {
        let group: Option<String> = dict.get("agent_group").and_then(|v| v.try_to().ok());
        if let Some(group) = group.filter(|s| !s.is_empty()) {
            return Some(Self::AgentGroupProp {
                group,
                prop: req_string(dict, "prop")?,
            });
        }
        if let Some(prop) = dict
            .get("agent")
            .and_then(|v| v.try_to::<String>().ok())
            .filter(|s| !s.is_empty())
        {
            return Some(Self::AgentProp { prop });
        }
        Some(Self::ObjectProp {
            uid: req_string(dict, "object")?,
            prop: req_string(dict, "prop")?,
        })
    }

    /// Writes the value; missing objects yield `false` and skip the op.
    fn write(
        &self,
        value: VariantSnapshot,
        agent_props: &mut HashMap<String, VariantSnapshot>,
        agent_objects: &mut HashMap<String, SimObjectData>,
        world_objects: &mut HashMap<String, SimObjectData>,
    ) {
        match self {
            Self::AgentProp { prop } => {
                agent_props.insert(prop.clone(), value);
            }
            Self::AgentGroupProp { group, prop } => {
                let uid = agent_objects
                    .iter()
                    .find(|(_, obj)| obj.groups.contains(group))
                    .map(|(uid, _)| uid.clone());
                if let Some(uid) = uid
                    && let Some(obj) = agent_objects.get_mut(&uid)
                {
                    obj.properties.insert(prop.clone(), value);
                }
            }
            Self::ObjectProp { uid, prop } => {
                if let Some(obj) = world_objects.get_mut(uid) {
                    obj.properties.insert(prop.clone(), value);
                }
            }
        }
    }
}

impl EffectAmount {
    /// Parses the `amount` of a dec: a finite number, or a lookup table dictionary.
    fn parse(value: &Variant) -> Option<Self> {
        if let Some(amount) = finite(variant_f64(value)) {
            return Some(Self::Direct(amount));
        }
        let dict: VarDictionary = value.try_to().ok()?;
        let binding = req_string(&dict, "binding")?;
        let table_raw: VarDictionary = dict.get("table").and_then(|v| v.try_to().ok())?;
        let mut table = HashMap::new();
        for (key, val) in table_raw.iter_shared() {
            let name: String = key.try_to().ok()?;
            if name.is_empty() {
                return None;
            }
            table.insert(name, finite(variant_f64(&val))?);
        }
        let default = finite(dict.get("default").and_then(|v| variant_f64(&v)))?;
        Some(Self::Lookup {
            binding,
            table,
            default,
        })
    }

    /// Resolves the amount against the (binding-injected) agent properties. A bound-but-absent
    /// key returns `None`, aborting the remaining ops like the GDScript early return.
    fn resolve(&self, agent_props: &HashMap<String, VariantSnapshot>) -> Option<f64> {
        match self {
            Self::Direct(amount) => Some(*amount),
            Self::Lookup {
                binding,
                table,
                default,
            } => match agent_props.get(binding) {
                Some(VariantSnapshot::Str(key)) if !key.is_empty() => table.get(key).copied(),
                Some(VariantSnapshot::Int(key)) => table.get(&key.to_string()).copied(),
                _ => Some(*default),
            },
        }
    }
}

/// Numeric value of an int or float snapshot; `None` for every other tier.
fn snap_as_f64(value: &VariantSnapshot) -> Option<f64> {
    match value {
        VariantSnapshot::Int(i) => Some(*i as f64),
        VariantSnapshot::Float(bits) => Some(f64::from_bits(*bits)),
        _ => None,
    }
}

/// A world object referred to by a binding value: instance id, uid string or object handle.
fn resolve_object<'a>(
    key: &VariantSnapshot,
    world_objects: &'a HashMap<String, SimObjectData>,
) -> Option<&'a SimObjectData> {
    match key {
        VariantSnapshot::ObjectRef(id) => world_objects.get(&id.to_string()),
        VariantSnapshot::Int(id) => world_objects.get(&id.to_string()),
        VariantSnapshot::Str(uid) => world_objects.get(uid),
        _ => None,
    }
}

impl EffectOp {
    /// Deserialises one spec entry. Returns `None` (without logging) for unknown ops, missing
    /// keys, non-finite floats, non-Tier-1 values and empty names; the caller falls back to the
    /// effect callable and warns once per action.
    pub fn from_dict(dict: &VarDictionary) -> Option<Self> {
        let op: String = dict.get("op")?.try_to().ok()?;
        match op.as_str() {
            "set" => {
                let target = EffectTarget::from_op_dict(dict)?;
                let raw = dict.get("value")?;
                Some(Self::Set {
                    target,
                    value: tier1(&VariantSnapshot::from_variant(&raw))?,
                })
            }
            "inc" => {
                let target = EffectTarget::from_op_dict(dict)?;
                let amount = finite(dict.get("amount").and_then(|v| variant_f64(&v)))?;
                Some(Self::Inc { target, amount })
            }
            "dec" => {
                let target = EffectTarget::from_op_dict(dict)?;
                let raw = dict.get("amount")?;
                Some(Self::Dec {
                    target,
                    amount: EffectAmount::parse(&raw)?,
                })
            }
            "min_cap" => {
                let target = EffectTarget::from_op_dict(dict)?;
                let cap = finite(dict.get("cap").and_then(|v| variant_f64(&v)))?;
                Some(Self::MinCap { target, cap })
            }
            "max_floor" => {
                let target = EffectTarget::from_op_dict(dict)?;
                let floor = finite(dict.get("floor").and_then(|v| variant_f64(&v)))?;
                Some(Self::MaxFloor { target, floor })
            }
            "copy" => {
                let from_raw = dict.get("from")?;
                let to_raw = dict.get("to")?;
                let from_dict: VarDictionary = from_raw.try_to().ok()?;
                let to_dict: VarDictionary = to_raw.try_to().ok()?;
                Some(Self::Copy {
                    from: CopySource::parse(&from_dict)?,
                    to: CopyDest::parse(&to_dict)?,
                })
            }
            _ => None,
        }
    }

    /// Applies one op to the cloned maps. Returns `false` when a `Lookup` missed, aborting the
    /// remaining ops; every other outcome (including skipped missing targets) returns `true`.
    fn apply_to(
        &self,
        agent_props: &mut HashMap<String, VariantSnapshot>,
        agent_objects: &mut HashMap<String, SimObjectData>,
        world_objects: &mut HashMap<String, SimObjectData>,
    ) -> bool {
        match self {
            Self::Set { target, value } => {
                if let Some(slot) = target.resolve_or_create(agent_props, world_objects) {
                    *slot = value.clone();
                }
                true
            }
            Self::Inc { target, amount } => {
                if let Some(slot) = target.resolve_existing(agent_props, world_objects)
                    && let Some(current) = snap_as_f64(slot)
                {
                    *slot = VariantSnapshot::Float((current + amount).to_bits());
                }
                true
            }
            Self::Dec { target, amount } => {
                let Some(subtract) = amount.resolve(agent_props) else {
                    return false;
                };
                if let Some(slot) = target.resolve_existing(agent_props, world_objects)
                    && let Some(current) = snap_as_f64(slot)
                {
                    *slot = VariantSnapshot::Float((current - subtract).to_bits());
                }
                true
            }
            Self::MinCap { target, cap } => {
                if let Some(slot) = target.resolve_existing(agent_props, world_objects)
                    && let Some(current) = snap_as_f64(slot)
                {
                    *slot = VariantSnapshot::Float(current.min(*cap).to_bits());
                }
                true
            }
            Self::MaxFloor { target, floor } => {
                if let Some(slot) = target.resolve_existing(agent_props, world_objects)
                    && let Some(current) = snap_as_f64(slot)
                {
                    *slot = VariantSnapshot::Float(current.max(*floor).to_bits());
                }
                true
            }
            Self::Copy { from, to } => {
                if let Some(value) = from.read(agent_props, world_objects) {
                    to.write(value, agent_props, agent_objects, world_objects);
                }
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn float(value: f64) -> VariantSnapshot {
        VariantSnapshot::Float(value.to_bits())
    }

    fn agent(props: &[(&str, VariantSnapshot)]) -> BlackboardSnapshot {
        BlackboardSnapshot {
            properties: props
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
            objects: HashMap::new(),
        }
    }

    fn agent_with_location(position: VariantSnapshot) -> BlackboardSnapshot {
        let mut objects = HashMap::new();
        objects.insert(
            "loc".to_string(),
            SimObjectData {
                uid: "loc".to_string(),
                groups: vec!["GdPAILocationData".to_string()],
                properties: HashMap::from([("position".to_string(), position)]),
            },
        );
        BlackboardSnapshot {
            properties: HashMap::new(),
            objects,
        }
    }

    fn world_with(uid: &str, props: &[(&str, VariantSnapshot)]) -> BlackboardSnapshot {
        let mut objects = HashMap::new();
        objects.insert(
            uid.to_string(),
            SimObjectData {
                uid: uid.to_string(),
                groups: vec![],
                properties: props
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.clone()))
                    .collect(),
            },
        );
        BlackboardSnapshot {
            properties: HashMap::new(),
            objects,
        }
    }

    fn no_bindings() -> Vec<(String, Vec<VariantSnapshot>)> {
        Vec::new()
    }

    /// `set` writes Tier-1 values to agent props, creating missing keys.
    #[test]
    fn set_writes_agent_prop() {
        let ops = vec![
            EffectOp::Set {
                target: EffectTarget::AgentProp {
                    prop: "held_item".to_string(),
                },
                value: VariantSnapshot::Str("wood".to_string()),
            },
            EffectOp::Set {
                target: EffectTarget::AgentProp {
                    prop: "wandered".to_string(),
                },
                value: VariantSnapshot::Bool(true),
            },
        ];
        let (after, _) = apply_effect_spec(&ops, &agent(&[]), &agent(&[]), &no_bindings());
        assert_eq!(
            after.properties.get("held_item"),
            Some(&VariantSnapshot::Str("wood".to_string()))
        );
        assert_eq!(
            after.properties.get("wandered"),
            Some(&VariantSnapshot::Bool(true))
        );
    }

    /// `set` on a world object writes through; a missing uid skips the op.
    #[test]
    fn set_writes_object_prop_and_skips_missing_uid() {
        let ops = vec![EffectOp::Set {
            target: EffectTarget::ObjectProp {
                uid: "7".to_string(),
                prop: "held_item".to_string(),
            },
            value: VariantSnapshot::Str("potato".to_string()),
        }];
        let world = world_with("7", &[("held_item", VariantSnapshot::Str(String::new()))]);
        let (_, after) = apply_effect_spec(&ops, &agent(&[]), &world, &no_bindings());
        assert_eq!(
            after.objects["7"].properties.get("held_item"),
            Some(&VariantSnapshot::Str("potato".to_string()))
        );

        let missing = vec![EffectOp::Set {
            target: EffectTarget::ObjectProp {
                uid: "gone".to_string(),
                prop: "held_item".to_string(),
            },
            value: VariantSnapshot::Str("potato".to_string()),
        }];
        let (agent_after, world_after) =
            apply_effect_spec(&missing, &agent(&[]), &world, &no_bindings());
        assert_eq!(agent_after, agent(&[]));
        assert_eq!(world_after.objects, world.objects);
    }

    /// `inc` adds and `min_cap` clamps: the AddFuel shape (90 + 15 capped at 100).
    #[test]
    fn inc_then_min_cap_matches_add_fuel() {
        let target = EffectTarget::ObjectProp {
            uid: "42".to_string(),
            prop: "current_fuel".to_string(),
        };
        let ops = vec![
            EffectOp::Inc {
                target: target.clone(),
                amount: 15.0,
            },
            EffectOp::MinCap { target, cap: 100.0 },
        ];
        let world = world_with("42", &[("current_fuel", float(90.0))]);
        let (_, after) = apply_effect_spec(&ops, &agent(&[]), &world, &no_bindings());
        assert_eq!(
            after.objects["42"].properties.get("current_fuel"),
            Some(&float(100.0))
        );
    }

    /// `dec` with a direct amount and `max_floor`: the ShakeTree shape.
    #[test]
    fn dec_then_max_floor_matches_shake_tree() {
        let target = EffectTarget::AgentProp {
            prop: "hunger".to_string(),
        };
        let ops = vec![
            EffectOp::Dec {
                target: target.clone(),
                amount: EffectAmount::Direct(7.5),
            },
            EffectOp::MaxFloor { target, floor: 0.0 },
        ];
        let (after, _) = apply_effect_spec(
            &ops,
            &agent(&[("hunger", float(3.0))]),
            &agent(&[]),
            &no_bindings(),
        );
        assert_eq!(after.properties.get("hunger"), Some(&float(0.0)));

        let (after, _) = apply_effect_spec(
            &ops,
            &agent(&[("hunger", float(50.0))]),
            &agent(&[]),
            &no_bindings(),
        );
        assert_eq!(after.properties.get("hunger"), Some(&float(42.5)));
    }

    /// Numeric ops skip missing or non-numeric targets instead of inventing state.
    #[test]
    fn numeric_ops_skip_unusable_targets() {
        let target = EffectTarget::AgentProp {
            prop: "hunger".to_string(),
        };
        let ops = vec![
            EffectOp::Dec {
                target: target.clone(),
                amount: EffectAmount::Direct(5.0),
            },
            EffectOp::Inc {
                target: target.clone(),
                amount: 5.0,
            },
            EffectOp::MinCap {
                target: target.clone(),
                cap: 9.0,
            },
            EffectOp::MaxFloor { target, floor: 1.0 },
        ];
        let missing = agent(&[]);
        let (after, _) = apply_effect_spec(&ops, &missing, &agent(&[]), &no_bindings());
        assert_eq!(after, missing);

        let text = agent(&[("hunger", VariantSnapshot::Str("full".to_string()))]);
        let (after, _) = apply_effect_spec(&ops, &text, &agent(&[]), &no_bindings());
        assert_eq!(after, text);
    }

    /// `dec` with a lookup: bound key hits the table, unbound falls back to the default.
    #[test]
    fn lookup_dec_hits_table_or_default() {
        let ops = vec![EffectOp::Dec {
            target: EffectTarget::AgentProp {
                prop: "hunger".to_string(),
            },
            amount: EffectAmount::Lookup {
                binding: "held_item".to_string(),
                table: HashMap::from([("potato".to_string(), 25.0)]),
                default: 20.0,
            },
        }];
        let before = agent(&[("hunger", float(80.0))]);

        let bound = vec![(
            "held_item".to_string(),
            vec![VariantSnapshot::Str("potato".to_string())],
        )];
        let (after, _) = apply_effect_spec(&ops, &before, &agent(&[]), &bound);
        assert_eq!(after.properties.get("hunger"), Some(&float(55.0)));

        let (after, _) = apply_effect_spec(&ops, &before, &agent(&[]), &no_bindings());
        assert_eq!(after.properties.get("hunger"), Some(&float(60.0)));
    }

    /// A bound-but-absent lookup key aborts the remaining ops, mirroring the GDScript return.
    #[test]
    fn lookup_miss_aborts_remaining_ops() {
        let ops = vec![
            EffectOp::Dec {
                target: EffectTarget::AgentProp {
                    prop: "hunger".to_string(),
                },
                amount: EffectAmount::Lookup {
                    binding: "held_item".to_string(),
                    table: HashMap::from([("potato".to_string(), 25.0)]),
                    default: 20.0,
                },
            },
            EffectOp::Set {
                target: EffectTarget::AgentProp {
                    prop: "held_item".to_string(),
                },
                value: VariantSnapshot::Str(String::new()),
            },
        ];
        let before = agent(&[("hunger", float(80.0))]);
        let bound_wood = vec![(
            "held_item".to_string(),
            vec![VariantSnapshot::Str("wood".to_string())],
        )];
        let (after, _) = apply_effect_spec(&ops, &before, &agent(&[]), &bound_wood);
        // Bindings are injected (as the callback path does), but neither op applies.
        assert_eq!(after.properties.get("hunger"), Some(&float(80.0)));
        assert_eq!(
            after.properties.get("held_item"),
            Some(&VariantSnapshot::Str("wood".to_string()))
        );
    }

    /// `copy` from a bound object to the agent's grouped proxy: the GoTo shape.
    #[test]
    fn copy_binding_to_agent_group_moves_location() {
        let ops = vec![EffectOp::Copy {
            from: CopySource::BindingProp {
                binding: "at_target".to_string(),
                prop: "position".to_string(),
            },
            to: CopyDest::AgentGroupProp {
                group: "GdPAILocationData".to_string(),
                prop: "position".to_string(),
            },
        }];
        let before = agent_with_location(VariantSnapshot::Str("here".to_string()));
        let world = world_with(
            "99",
            &[("position", VariantSnapshot::Str("there".to_string()))],
        );
        let bound = vec![(
            "at_target".to_string(),
            vec![VariantSnapshot::ObjectRef(99)],
        )];
        let (after, world_after) = apply_effect_spec(&ops, &before, &world, &bound);
        assert_eq!(
            after.objects["loc"].properties.get("position"),
            Some(&VariantSnapshot::Str("there".to_string()))
        );
        assert_eq!(world_after.objects, world.objects);
    }

    /// `copy` resolves int and string binding keys, and no-ops when unbound or missing.
    #[test]
    fn copy_resolves_all_key_shapes_or_skips() {
        let ops = vec![EffectOp::Copy {
            from: CopySource::BindingProp {
                binding: "at_target".to_string(),
                prop: "position".to_string(),
            },
            to: CopyDest::AgentProp {
                prop: "echo".to_string(),
            },
        }];
        let world = world_with("7", &[("position", VariantSnapshot::Str("p".to_string()))]);
        for key in [
            VariantSnapshot::ObjectRef(7),
            VariantSnapshot::Int(7),
            VariantSnapshot::Str("7".to_string()),
        ] {
            let bound = vec![("at_target".to_string(), vec![key])];
            let (after, _) = apply_effect_spec(&ops, &agent(&[]), &world, &bound);
            assert_eq!(
                after.properties.get("echo"),
                Some(&VariantSnapshot::Str("p".to_string()))
            );
        }

        let before = agent(&[]);
        let (after, _) = apply_effect_spec(&ops, &before, &world, &no_bindings());
        assert_eq!(after, before);

        let dangling = vec![(
            "at_target".to_string(),
            vec![VariantSnapshot::ObjectRef(123)],
        )];
        let (after, _) = apply_effect_spec(&ops, &before, &world, &dangling);
        assert!(!after.properties.contains_key("echo"));
    }

    /// `copy` preserves Tier-2 positions byte-for-byte: the live GoTo case, where a
    /// Vector3 snapshots as `Bytes` and the requirement check compares exact bytes.
    /// (Parity audit for the Fire branch-count investigation: the copy must move the
    /// bytes unchanged, never re-encode or convert them.)
    #[test]
    fn copy_preserves_bytes_position_untouched() {
        let ops = vec![EffectOp::Copy {
            from: CopySource::BindingProp {
                binding: "at_target".to_string(),
                prop: "position".to_string(),
            },
            to: CopyDest::AgentGroupProp {
                group: "GdPAILocationData".to_string(),
                prop: "position".to_string(),
            },
        }];
        let target_pos = VariantSnapshot::Bytes(vec![9, 0, 0, 0, 0, 0, 0, 0]);
        let before = agent_with_location(VariantSnapshot::Bytes(vec![1, 2, 3, 4]));
        let world = world_with("99", &[("position", target_pos.clone())]);
        let bound = vec![(
            "at_target".to_string(),
            vec![VariantSnapshot::ObjectRef(99)],
        )];
        let (after, world_after) = apply_effect_spec(&ops, &before, &world, &bound);
        assert_eq!(
            after.objects["loc"].properties.get("position"),
            Some(&target_pos),
            "the copy must move the exact bytes, not a conversion"
        );
        assert_eq!(world_after.objects, world.objects);
        // A missing source property skips the copy instead of writing null.
        let bare = world_with("7", &[]);
        let bound_bare = vec![("at_target".to_string(), vec![VariantSnapshot::ObjectRef(7)])];
        let (after_bare, _) = apply_effect_spec(&ops, &before, &bare, &bound_bare);
        assert_eq!(
            after_bare.objects["loc"].properties.get("position"),
            Some(&VariantSnapshot::Bytes(vec![1, 2, 3, 4])),
            "missing source leaves the agent position alone"
        );
    }

    /// Unknown ops, missing keys, non-finite floats, non-Tier-1 values and empty names are
    /// all rejected by the pure parsers the dictionary rim delegates to.
    #[test]
    fn malformed_specs_are_rejected() {
        assert!(EffectTarget::parse("agent", None, Some("hunger")).is_some());
        assert!(EffectTarget::parse("object", Some("7"), Some("fuel")).is_some());
        assert!(EffectTarget::parse("object", None, Some("fuel")).is_none());
        assert!(EffectTarget::parse("object", Some(""), Some("fuel")).is_none());
        assert!(EffectTarget::parse("agent", None, Some("")).is_none());
        assert!(EffectTarget::parse("agent", None, None).is_none());
        assert!(EffectTarget::parse("nowhere", None, Some("hunger")).is_none());

        assert!(finite(Some(1.5)).is_some());
        assert!(finite(Some(f64::NAN)).is_none());
        assert!(finite(Some(f64::INFINITY)).is_none());
        assert!(finite(None).is_none());

        assert!(tier1(&VariantSnapshot::Bool(true)).is_some());
        assert!(tier1(&VariantSnapshot::Int(3)).is_some());
        assert!(tier1(&float(1.5)).is_some());
        assert!(tier1(&VariantSnapshot::Str("x".to_string())).is_some());
        assert!(tier1(&float(f64::NAN)).is_none());
        assert!(tier1(&VariantSnapshot::Nil).is_none());
        assert!(tier1(&VariantSnapshot::Bytes(vec![1])).is_none());
        assert!(tier1(&VariantSnapshot::ObjectRef(1)).is_none());
        assert!(tier1(&VariantSnapshot::Array(vec![])).is_none());
    }

    /// EatHeldFood parity: bound potato, optimistic unbound default, miss aborts.
    #[test]
    fn eat_held_food_parity_fixture() {
        let ops = vec![
            EffectOp::Dec {
                target: EffectTarget::AgentProp {
                    prop: "hunger".to_string(),
                },
                amount: EffectAmount::Lookup {
                    binding: "held_item".to_string(),
                    table: HashMap::from([
                        ("potato".to_string(), 10.0),
                        ("cooked_potato".to_string(), 35.0),
                    ]),
                    default: 20.0,
                },
            },
            EffectOp::MaxFloor {
                target: EffectTarget::AgentProp {
                    prop: "hunger".to_string(),
                },
                floor: 0.0,
            },
            EffectOp::Set {
                target: EffectTarget::AgentProp {
                    prop: "held_item".to_string(),
                },
                value: VariantSnapshot::Str(String::new()),
            },
        ];
        // Bound cooked potato: hunger drops by the table value, slot clears.
        let before = agent(&[("hunger", float(50.0))]);
        let bound = vec![(
            "held_item".to_string(),
            vec![VariantSnapshot::Str("cooked_potato".to_string())],
        )];
        let (after, _) = apply_effect_spec(&ops, &before, &agent(&[]), &bound);
        assert_eq!(after.properties.get("hunger"), Some(&float(15.0)));
        assert_eq!(
            after.properties.get("held_item"),
            Some(&VariantSnapshot::Str(String::new()))
        );

        // Unbound discovery: the optimistic default still floors at zero.
        let hungry = agent(&[("hunger", float(10.0))]);
        let (after, _) = apply_effect_spec(&ops, &hungry, &agent(&[]), &no_bindings());
        assert_eq!(after.properties.get("hunger"), Some(&float(0.0)));
        assert_eq!(
            after.properties.get("held_item"),
            Some(&VariantSnapshot::Str(String::new()))
        );
    }

    /// Missing hunger still clears the hand: Dec/MaxFloor skip a missing target while Set
    /// applies. Unreachable-but-defined: validity requires agent_has_property hunger and no
    /// action deletes hunger, so planning never simulates this; the GDScript body instead
    /// returns early without clearing. Pinned here so any reachability change is deliberate.
    #[test]
    fn eat_missing_hunger_clears_hand_but_keeps_hunger_absent() {
        let ops = vec![
            EffectOp::Dec {
                target: EffectTarget::AgentProp {
                    prop: "hunger".to_string(),
                },
                amount: EffectAmount::Lookup {
                    binding: "held_item".to_string(),
                    table: HashMap::from([("potato".to_string(), 10.0)]),
                    default: 20.0,
                },
            },
            EffectOp::MaxFloor {
                target: EffectTarget::AgentProp {
                    prop: "hunger".to_string(),
                },
                floor: 0.0,
            },
            EffectOp::Set {
                target: EffectTarget::AgentProp {
                    prop: "held_item".to_string(),
                },
                value: VariantSnapshot::Str(String::new()),
            },
        ];
        let before = agent(&[("held_item", VariantSnapshot::Str("potato".to_string()))]);
        let (after, _) = apply_effect_spec(&ops, &before, &agent(&[]), &no_bindings());
        assert_eq!(after.properties.get("hunger"), None);
        assert_eq!(
            after.properties.get("held_item"),
            Some(&VariantSnapshot::Str(String::new()))
        );
    }
}
