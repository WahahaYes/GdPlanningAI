//! Requirements and provisions: the symbolic causal links of the backward search.
//!
//! [`provision_satisfies_requirement`] matches them while building a chain;
//! [`requirement_holds_in_state`] re-checks requirements during verification. A wildcard
//! provision such as GoTo's `FactWildcard("at_target")` matches any `Fact("at_target", args)` and
//! binds those `args` to the provider.

use crate::snapshot::VariantSnapshot;
use godot::prelude::*;

/// A planner-readable dependency that must be satisfied by a prior action's provisions.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RequirementSpec {
    /// A binding with this name must have been provided (or the agent already has a non-empty
    /// property of that name).
    BindingExists {
        /// Name of the binding.
        binding_name: String,
    },
    /// A binding with this name must have exactly this value.
    BindingEquals {
        /// Name of the binding.
        binding_name: String,
        /// Required value.
        value: VariantSnapshot,
    },
    /// A binding with this name must refer to a world object that belongs to a group.
    BindingInSet {
        /// Name of the binding; its value is an object handle.
        binding_name: String,
        /// Name of the group the object must be in.
        set_name: String,
    },
    /// A named fact must hold, optionally for specific arguments.
    Fact {
        /// Name of the fact.
        fact_name: String,
        /// Arguments the fact must have; empty accepts any.
        args: Vec<VariantSnapshot>,
    },
}

/// A value or fact that an action contributes for later actions to consume.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProvisionSpec {
    /// Binds `binding_name` to a concrete value.
    Binding {
        /// Name of the binding.
        binding_name: String,
        /// The bound value.
        value: VariantSnapshot,
    },
    /// Provides a fact with concrete arguments.
    Fact {
        /// Name of the fact.
        fact_name: String,
        /// The fact's arguments.
        args: Vec<VariantSnapshot>,
    },
    /// Provides the named fact for *any* arguments. The arguments are only decided when it is
    /// matched with a requirement (see the module docs).
    FactWildcard {
        /// Name of the fact.
        fact_name: String,
    },
}

impl RequirementSpec {
    /// Deserialises a [`RequirementSpec`] from a GDScript dictionary.
    ///
    /// The `kind` key selects the variant (`binding_exists`, `binding_equals`,
    /// `binding_in_set`, `fact`); the other keys follow the variant's field names. Returns
    /// `None` (with a warning for unknown kinds) when a required key is missing. Must be called
    /// on the main thread because values are snapshotted.
    pub fn from_dict(dict: &VarDictionary) -> Option<Self> {
        let kind = dict
            .get("kind")
            .and_then(|v| v.try_to::<String>().ok())
            .unwrap_or_default();

        match kind.as_str() {
            "binding_exists" => {
                let binding_name = dict
                    .get("binding_name")
                    .and_then(|v| v.try_to::<String>().ok())?;
                Some(Self::BindingExists { binding_name })
            }
            "binding_equals" => {
                let binding_name = dict
                    .get("binding_name")
                    .and_then(|v| v.try_to::<String>().ok())?;
                let value = dict.get("value")?;
                Some(Self::BindingEquals {
                    binding_name,
                    value: VariantSnapshot::from_variant(&value),
                })
            }
            "binding_in_set" => {
                let binding_name = dict
                    .get("binding_name")
                    .and_then(|v| v.try_to::<String>().ok())?;
                let set_name = dict
                    .get("set_name")
                    .and_then(|v| v.try_to::<String>().ok())?;
                Some(Self::BindingInSet {
                    binding_name,
                    set_name,
                })
            }
            "fact" => {
                let fact_name = dict
                    .get("fact_name")
                    .and_then(|v| v.try_to::<String>().ok())?;
                Some(Self::Fact {
                    fact_name,
                    args: extract_variant_snapshots(dict, "args"),
                })
            }
            _ => {
                log_warn!("RequirementSpec: unrecognised kind '{}'", kind);
                None
            }
        }
    }
}

impl std::fmt::Display for RequirementSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BindingExists { binding_name } => write!(f, "exists({})", binding_name),
            Self::BindingEquals {
                binding_name,
                value,
            } => write!(f, "{} == {:?}", binding_name, value),
            Self::BindingInSet {
                binding_name,
                set_name,
            } => write!(f, "{} in {}", binding_name, set_name),
            Self::Fact { fact_name, args } => {
                if args.is_empty() {
                    write!(f, "{}", fact_name)
                } else {
                    write!(f, "{}({:?})", fact_name, args)
                }
            }
        }
    }
}

impl std::hash::Hash for ProvisionSpec {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::Binding {
                binding_name,
                value,
            } => {
                0.hash(state);
                binding_name.hash(state);
                value.hash(state);
            }
            Self::Fact { fact_name, args } => {
                1.hash(state);
                fact_name.hash(state);
                args.hash(state);
            }
            Self::FactWildcard { fact_name } => {
                2.hash(state);
                fact_name.hash(state);
            }
        }
    }
}

impl ProvisionSpec {
    /// Deserialises a [`ProvisionSpec`] from a GDScript dictionary.
    ///
    /// The `kind` key selects the variant (`binding`, `fact`, `fact_wildcard`); the other keys
    /// follow the variant's field names. Returns `None` (with a warning for unknown kinds) when
    /// a required key is missing. Must be called on the main thread because values are
    /// snapshotted.
    pub fn from_dict(dict: &VarDictionary) -> Option<Self> {
        let kind = dict
            .get("kind")
            .and_then(|v| v.try_to::<String>().ok())
            .unwrap_or_default();

        match kind.as_str() {
            "binding" => {
                let binding_name = dict
                    .get("binding_name")
                    .and_then(|v| v.try_to::<String>().ok())?;
                let value = dict.get("value")?;
                Some(Self::Binding {
                    binding_name,
                    value: VariantSnapshot::from_variant(&value),
                })
            }
            "fact" => {
                let fact_name = dict
                    .get("fact_name")
                    .and_then(|v| v.try_to::<String>().ok())?;
                Some(Self::Fact {
                    fact_name,
                    args: extract_variant_snapshots(dict, "args"),
                })
            }
            "fact_wildcard" => {
                let fact_name = dict
                    .get("fact_name")
                    .and_then(|v| v.try_to::<String>().ok())?;
                Some(Self::FactWildcard { fact_name })
            }
            _ => {
                log_warn!("ProvisionSpec: unrecognised kind '{}'", kind);
                None
            }
        }
    }
}

/// Turns every non-empty agent property into a `Binding` provision of the same name.
pub fn extract_initial_provisions(
    agent: &crate::snapshot::BlackboardSnapshot,
) -> Vec<ProvisionSpec> {
    let mut provisions = Vec::new();

    // Extract all non-null agent properties as binding provisions
    for (key, value) in &agent.properties {
        // Skip null/empty values that don't represent meaningful bindings
        if !value.is_null() && !value.is_empty_string() {
            provisions.push(ProvisionSpec::Binding {
                binding_name: key.clone(),
                value: value.clone(),
            });
        }
    }

    provisions
}

/// Returns `true` if `requirement` holds in the simulated state. Only bindings and the
/// `at_target` fact are checked; other facts are assumed backed by preconditions. For
/// `at_target` the agent's simulated location wins over `current_bindings`, so a later GoTo
/// invalidates an earlier one.
pub fn requirement_holds_in_state(
    requirement: &RequirementSpec,
    agent: &crate::snapshot::BlackboardSnapshot,
    world: &crate::snapshot::BlackboardSnapshot,
    current_bindings: &[(String, Vec<crate::snapshot::VariantSnapshot>)],
) -> bool {
    match requirement {
        RequirementSpec::BindingExists { binding_name } => {
            if current_bindings
                .iter()
                .any(|(n, vals)| n == binding_name && !vals.is_empty())
            {
                return true;
            }
            agent
                .properties
                .get(binding_name)
                .is_some_and(|v| !v.is_null() && !v.is_empty_string())
        }
        RequirementSpec::BindingEquals {
            binding_name,
            value,
        } => {
            if current_bindings
                .iter()
                .any(|(n, vals)| n == binding_name && vals.len() == 1 && &vals[0] == value)
            {
                return true;
            }
            agent.properties.get(binding_name) == Some(value)
        }
        RequirementSpec::BindingInSet {
            binding_name,
            set_name,
        } => {
            if current_bindings.iter().any(|(n, vals)| {
                n == binding_name
                    && !vals.is_empty()
                    && vals.first().is_some_and(|value| {
                        if let crate::snapshot::VariantSnapshot::ObjectRef(id) = value
                            && let Some(obj_data) = world.get_object_by_instance_id(*id)
                        {
                            return obj_data.groups.contains(set_name);
                        }
                        false
                    })
            }) {
                return true;
            }
            agent.properties.get(binding_name).is_some_and(|value| {
                if let crate::snapshot::VariantSnapshot::ObjectRef(id) = value
                    && let Some(obj_data) = world.get_object_by_instance_id(*id)
                {
                    return obj_data.groups.contains(set_name);
                }
                false
            })
        }
        RequirementSpec::Fact { fact_name, args } => {
            if fact_name == "at_target" && !args.is_empty() {
                let target_pos = if let crate::snapshot::VariantSnapshot::ObjectRef(id) = &args[0] {
                    world
                        .get_object_by_instance_id(*id)
                        .and_then(|obj| obj.properties.get("position"))
                } else {
                    None
                };
                let agent_loc = agent.get_object_by_group("GdPAILocationData");
                let agent_pos = agent_loc.and_then(|obj| obj.properties.get("position"));
                if let (Some(tp), Some(ap)) = (target_pos, agent_pos) {
                    return tp == ap;
                }
                // No agent location state available; fall back to the bound value.
                if let Some((_, vals)) = current_bindings.iter().find(|(n, _)| n == fact_name) {
                    return vals == args;
                }
                return false;
            }
            // State-less or otherwise unknown facts cannot be validated here; assume true
            // rather than rejecting plans. In practice these are covered by preconditions.
            true
        }
    }
}

/// Returns `true` if `provision` satisfies `requirement`: same kind and name, matching value or
/// arguments (empty requirement arguments match any), or a `FactWildcard` of the same name.
/// `BindingInSet` also checks group membership when `world` is given.
pub fn provision_satisfies_requirement(
    provision: &ProvisionSpec,
    requirement: &RequirementSpec,
    world: Option<&crate::snapshot::BlackboardSnapshot>,
) -> bool {
    match (provision, requirement) {
        (
            ProvisionSpec::Binding {
                binding_name: provided_name,
                ..
            },
            RequirementSpec::BindingExists { binding_name },
        ) => provided_name == binding_name,
        (
            ProvisionSpec::Binding {
                binding_name: provided_name,
                value: provided_value,
            },
            RequirementSpec::BindingEquals {
                binding_name,
                value,
            },
        ) => provided_name == binding_name && provided_value == value,
        (
            ProvisionSpec::Binding {
                binding_name: provided_name,
                value: provided_value,
            },
            RequirementSpec::BindingInSet {
                binding_name,
                set_name,
            },
        ) => {
            if provided_name != binding_name {
                return false;
            }

            // If we have world context, check if the provided object is in the requested set
            if let Some(w) = world {
                if let crate::snapshot::VariantSnapshot::ObjectRef(id) = provided_value {
                    let uid = id.to_string();
                    if let Some(obj_data) = w.objects.get(&uid) {
                        return obj_data.groups.contains(set_name);
                    }
                }
                // If world is provided but object isn't found or isn't an ObjectRef, it fails.
                return false;
            }

            // Fallback: if no world context, we treat as satisfied
            // only if the binding names match.
            provided_name == binding_name
        }
        (
            ProvisionSpec::Fact {
                fact_name: provided_name,
                args: provided_args,
            },
            RequirementSpec::Fact { fact_name, args },
        ) => provided_name == fact_name && (args.is_empty() || provided_args == args),
        (
            ProvisionSpec::FactWildcard {
                fact_name: provided_name,
            },
            RequirementSpec::Fact { fact_name, .. },
        ) => provided_name == fact_name,
        _ => false,
    }
}

/// Reads `dict[key]` as an array and snapshots each element; a missing key gives an empty
/// list. Accepts typed and untyped arrays.
fn extract_variant_snapshots(dict: &VarDictionary, key: &str) -> Vec<VariantSnapshot> {
    dict.get(key)
        .and_then(|v| {
            v.try_to::<Array<Variant>>().ok().or_else(|| {
                v.try_to::<VarArray>().ok().map(|arr| {
                    let mut typed = Array::<Variant>::new();
                    for item in arr.iter_shared() {
                        typed.push(&item);
                    }
                    typed
                })
            })
        })
        .map(|arr| {
            arr.iter_shared()
                .map(|value| VariantSnapshot::from_variant(&value))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{BlackboardSnapshot, SimObjectData, VariantSnapshot};
    use std::collections::HashMap;

    #[test]
    fn binding_in_set_checks_all_matching_bindings() {
        let agent = BlackboardSnapshot {
            properties: HashMap::new(),
            objects: HashMap::new(),
        };
        let mut world = BlackboardSnapshot {
            properties: HashMap::new(),
            objects: HashMap::new(),
        };
        world.objects.insert(
            "1".to_string(),
            SimObjectData {
                uid: "1".to_string(),
                groups: vec!["weapon".to_string()],
                properties: HashMap::new(),
            },
        );
        world.objects.insert(
            "2".to_string(),
            SimObjectData {
                uid: "2".to_string(),
                groups: vec!["food".to_string()],
                properties: HashMap::new(),
            },
        );

        let current_bindings = vec![
            ("held_item".to_string(), vec![VariantSnapshot::ObjectRef(1)]),
            ("held_item".to_string(), vec![VariantSnapshot::ObjectRef(2)]),
        ];

        let req = RequirementSpec::BindingInSet {
            binding_name: "held_item".to_string(),
            set_name: "food".to_string(),
        };

        assert!(
            requirement_holds_in_state(&req, &agent, &world, &current_bindings),
            "BindingInSet should pass when any matching binding is in the requested set"
        );
    }
}
