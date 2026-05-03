//! Requirement and provision specs for planner-readable dependency chaining.
//!
//! These are deserialized from built-in Godot wrapper classes such as
//! `RequirementSpec.binding_exists(...)` and `ProvisionSpec.fact(...)`.

use crate::snapshot::VariantSnapshot;
use godot::prelude::*;

/// A planner-readable dependency that must be satisfied by a prior action's provisions.
#[derive(Clone, Debug, PartialEq)]
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
#[derive(Clone, Debug, PartialEq)]
pub enum ProvisionSpec {
    Binding {
        binding_name: String,
        value: VariantSnapshot,
    },
    Fact {
        fact_name: String,
        args: Vec<VariantSnapshot>,
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
        .any(|provision| provision_satisfies_requirement(provision, requirement))
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
pub fn extract_initial_provisions(agent: &crate::snapshot::BlackboardSnapshot) -> Vec<ProvisionSpec> {
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

/// Returns `true` if a provision satisfies a requirement.
pub fn provision_satisfies_requirement(
    provision: &ProvisionSpec,
    requirement: &RequirementSpec,
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
                ..
            },
            RequirementSpec::BindingInSet { binding_name, .. },
        ) => provided_name == binding_name,
        (
            ProvisionSpec::Fact {
                fact_name: provided_name,
                args: provided_args,
            },
            RequirementSpec::Fact { fact_name, args },
        ) => provided_name == fact_name && provided_args == args,
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
