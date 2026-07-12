//! Requirement and provision specs for planner-readable dependency chaining.
//!
//! These are deserialized from built-in Godot wrapper classes such as
//! `RequirementSpec.binding_exists(...)` and `ProvisionSpec.fact(...)`.

use crate::snapshot::VariantSnapshot;
use godot::prelude::*;

/// A planner-readable dependency that must be satisfied by a prior action's provisions.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RequirementSpec {
    BindingExists {
        binding_name: String,
    },
    BindingEquals {
        binding_name: String,
        value: VariantSnapshot,
    },
    BindingInSet {
        binding_name: String,
        set_name: String,
    },
    Fact {
        fact_name: String,
        args: Vec<VariantSnapshot>,
    },
}

/// A value or fact that an action contributes for later actions to consume.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProvisionSpec {
    Binding {
        binding_name: String,
        value: VariantSnapshot,
    },
    Fact {
        fact_name: String,
        args: Vec<VariantSnapshot>,
    },
    FactWildcard {
        fact_name: String,
    },
}

impl RequirementSpec {
    /// Deserialises a [`RequirementSpec`] from a GDScript dictionary.
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

/// Returns `true` if every requirement in `requirements` is satisfied by at least one provision.
pub fn requirements_satisfied(
    requirements: &[RequirementSpec],
    provisions: &[ProvisionSpec],
) -> bool {
    requirements
        .iter()
        .all(|requirement| requirement_satisfied(requirement, provisions))
}

/// Returns `true` if `requirement` is satisfied by at least one entry in `provisions`.
pub fn requirement_satisfied(requirement: &RequirementSpec, provisions: &[ProvisionSpec]) -> bool {
    provisions
        .iter()
        .any(|provision| provision_satisfies_requirement(provision, requirement, None))
}

/// Returns `true` if any requirement in `requirements` is satisfied by `provisions`.
pub fn provisions_satisfy_any_requirement(
    provisions: &[ProvisionSpec],
    requirements: &[RequirementSpec],
) -> bool {
    requirements
        .iter()
        .any(|requirement| requirement_satisfied(requirement, provisions))
}

/// Returns a filtered copy of `requirements` with any already-satisfied entries removed.
pub fn remove_satisfied_requirements(
    requirements: &[RequirementSpec],
    provisions: &[ProvisionSpec],
) -> Vec<RequirementSpec> {
    requirements
        .iter()
        .filter(|requirement| !requirement_satisfied(requirement, provisions))
        .cloned()
        .collect()
}

/// Appends entries from `additional` into `requirements`, skipping duplicates.
pub fn extend_unique_requirements(
    requirements: &mut Vec<RequirementSpec>,
    additional: &[RequirementSpec],
) {
    for requirement in additional {
        if !requirements.contains(requirement) {
            requirements.push(requirement.clone());
        }
    }
}

/// Appends entries from `additional` into `provisions`, skipping duplicates.
pub fn extend_unique_provisions(provisions: &mut Vec<ProvisionSpec>, additional: &[ProvisionSpec]) {
    for provision in additional {
        if !provisions.contains(provision) {
            provisions.push(provision.clone());
        }
    }
}

/// Returns the subset of `requirements` that are NOT satisfied by `provisions`.
///
/// This is useful for determining which requirements remain unresolved
/// before deciding whether an action can be concretely simulated.
pub fn get_unsatisfied_requirements(
    requirements: &[RequirementSpec],
    provisions: &[ProvisionSpec],
) -> Vec<RequirementSpec> {
    requirements
        .iter()
        .filter(|req| !requirement_satisfied(req, provisions))
        .cloned()
        .collect()
}

/// Extract initial provisions from the current agent state.
///
/// This allows existing bindings in the agent state to satisfy requirements
/// without needing explicit action provisions.
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

/// Returns `true` if `requirement` is currently satisfied by the given `agent` and `world` state,
/// optionally considering the `current_bindings` for the action being validated.
///
/// `current_bindings` are the per-chain-position bindings for the current action. For `Binding`
/// requirements and the `at_target` fact, they represent the concrete value a predecessor action
/// has promised. The actual `agent` state (e.g. `agent_location` position) is preferred for
/// `at_target` so that a later Go To which moved the agent correctly invalidates an earlier one.
/// If no state information is available, the bindings are used as a fallback so forward
/// validation still works for actions whose `simulate_effect` does not explicitly set a property.
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
                .map_or(false, |v| !v.is_null() && !v.is_empty_string())
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
            agent
                .properties
                .get(binding_name)
                .map_or(false, |v| v == value)
        }
        RequirementSpec::BindingInSet {
            binding_name,
            set_name,
        } => {
            if current_bindings.iter().any(|(n, vals)| {
                n == binding_name
                    && !vals.is_empty()
                    && vals.first().map_or(false, |value| {
                        if let crate::snapshot::VariantSnapshot::ObjectRef(id) = value {
                            if let Some(obj_data) = world.get_object_by_instance_id(*id) {
                                return obj_data.groups.contains(set_name);
                            }
                        }
                        false
                    })
            }) {
                return true;
            }
            agent.properties.get(binding_name).map_or(false, |value| {
                if let crate::snapshot::VariantSnapshot::ObjectRef(id) = value {
                    if let Some(obj_data) = world.get_object_by_instance_id(*id) {
                        return obj_data.groups.contains(set_name);
                    }
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

/// Returns `true` if a provision satisfies a requirement.
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
