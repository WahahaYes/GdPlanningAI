use crate::plan_types::*;
use crate::snapshot::{BlackboardSnapshot, VariantSnapshot};
use crate::requirement::{ProvisionSpec, RequirementSpec};
use super::types::{PlanBranch, ActionCandidate};
use super::simulation;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::collections::HashMap;

pub struct SearchContext {
    pub actions: Vec<ActionSpec>,
    pub initial_agent: BlackboardSnapshot,
    pub initial_world: BlackboardSnapshot,
    pub initial_provisions: Vec<ProvisionSpec>,
    pub request_tx: Sender<CallbackRequest>,
    pub pending_requests: std::sync::Mutex<HashMap<SimulationKey, usize>>,
    pub callback_results: std::sync::Mutex<HashMap<SimulationKey, CallbackResponse>>,
}

pub enum ExpandResult {
    Ready(Vec<PlanBranch>),
    Pending(usize),
}

pub struct BranchExpander {
    pub ctx: Arc<SearchContext>,
}

impl BranchExpander {
    pub fn expand(&self, branch: &PlanBranch) -> ExpandResult {
        let candidates = match self.find_candidates(branch) {
            Ok(c) => c,
            Err(id) => return ExpandResult::Pending(id),
        };
        
        let mut successors = Vec::new();
        for candidate in candidates {
            match self.expand_branch(branch, &candidate) {
                Ok(Some(new_branch)) => successors.push(new_branch),
                Ok(None) => {}
                Err(id) => return ExpandResult::Pending(id),
            }
        }

        ExpandResult::Ready(successors)
    }

    fn find_candidates(&self, branch: &PlanBranch) -> Result<Vec<ActionCandidate>, usize> {
        let mut candidates = Vec::new();
        let mut first_pending_id = None;

        log_debug!("find_candidates: Checking {} actions against {} preconds and {} requirements", self.ctx.actions.len(), branch.open_preconditions.len(), branch.open_requirements.len());

        for (idx, action) in self.ctx.actions.iter().enumerate() {
            // 0. Check Validity Checks (hard prerequisites against initial state)
            let mut valid = true;
            let mut action_pending_id = None;

            for check in &action.validity_checks {
                match simulation::eval_precondition(
                    check,
                    &self.ctx.initial_agent,
                    &self.ctx.initial_world,
                    self.ctx.initial_provisions.clone(),
                    vec![],
                    &*self.ctx,
                ) {
                    simulation::PreconditionResult::Ready(false) => {
                        log_debug!("Discovery: Action {} failed validity check, skipping", action.name);
                        valid = false;
                        break;
                    }
                    simulation::PreconditionResult::Pending(id) => {
                        action_pending_id = Some(id);
                        valid = false; // Treat as invalid for THIS frame
                        break;
                    }
                    _ => {}
                }
            }

            if let Some(id) = action_pending_id {
                if first_pending_id.is_none() { first_pending_id = Some(id); }
                continue; // Move to next action, but keep track of the yield requirement
            }

            if !valid {
                continue;
            }

            let mut satisfied_preconditions = Vec::new();
            let mut satisfied_requirements = Vec::new();

            // 1. Check Symbolic Requirements
            for (req_idx, (_, req)) in branch.open_requirements.iter().enumerate() {
                for prov in &action.provisions {
                    if crate::requirement::provision_satisfies_requirement(prov, req, Some(&self.ctx.initial_world)) {
                        if !satisfied_requirements.contains(&req_idx) {
                            satisfied_requirements.push(req_idx);
                        }
                    }
                }
            }

            // 2. Check Physical Preconditions (Optimistically)
            let mut hypothetical_agent = self.ctx.initial_agent.clone();
            let mut hypothetical_world = self.ctx.initial_world.clone();

            let branch_reqs: Vec<RequirementSpec> = branch.open_requirements.iter().map(|(_, r)| r.clone()).collect();
            apply_requirements_to_snapshots(&branch_reqs, &mut hypothetical_agent, &mut hypothetical_world);
            apply_requirements_to_snapshots(&action.requirements, &mut hypothetical_agent, &mut hypothetical_world);

            let mut sim_pending_id = None;
            for (pre_idx, precond) in branch.open_preconditions.iter().enumerate() {
                log_debug!("Discovery: Starting simulation for action {}", action.name);
                
                match simulation::simulate_action(
                    action,
                    0,
                    &[],
                    &hypothetical_agent,
                    &hypothetical_world,
                    self.ctx.initial_provisions.clone(),
                    &*self.ctx,
                ) {
                    simulation::SimulationStepResult::Ready(res) => {
                        log_debug!("Discovery: Simulation success for {}. Agent props: {:?}", action.name, res.agent.properties);

                        match simulation::eval_precondition(
                            precond,
                            &res.agent,
                            &res.world,
                            self.ctx.initial_provisions.clone(),
                            vec![],
                            &*self.ctx,
                        ) {
                            simulation::PreconditionResult::Ready(true) => {
                                log_debug!("Discovery: Action {} satisfied precond {:?}", action.name, precond);
                                satisfied_preconditions.push(pre_idx);
                            }
                            simulation::PreconditionResult::Pending(id) => {
                                sim_pending_id = Some(id);
                                break;
                            }
                            _ => {
                                log_debug!("Discovery: Action {} did NOT satisfy precond {:?}", action.name, precond);
                            }
                        }
                    }
                    simulation::SimulationStepResult::Pending(id) => {
                        sim_pending_id = Some(id);
                        break;
                    }
                    _ => {
                        log_debug!("Discovery: Action {} simulation RETURNED NONE", action.name);
                    }
                }
            }

            if let Some(id) = sim_pending_id {
                if first_pending_id.is_none() { first_pending_id = Some(id); }
                continue;
            }

            if !satisfied_preconditions.is_empty() || !satisfied_requirements.is_empty() {
                let mut est_cost = 1.0;
                if satisfied_preconditions.is_empty() {
                    est_cost += 0.1; 
                }

                log_debug!("Found candidate: {} (satisfied preconds: {:?}, requirements: {:?})", action.name, satisfied_preconditions, satisfied_requirements);
                candidates.push(ActionCandidate {
                    action_idx: idx,
                    estimated_cost: est_cost, 
                    satisfied_precondition_indices: satisfied_preconditions,
                    satisfied_requirement_indices: satisfied_requirements,
                });
            }
        }

        // If we found ANY valid candidates (cached or builtin), proceed!
        if !candidates.is_empty() {
            candidates.sort_by(|a, b| a.estimated_cost.partial_cmp(&b.estimated_cost).unwrap_or(std::cmp::Ordering::Equal));
            return Ok(candidates);
        }

        // If no progress possible but we have pending requests, yield the first one.
        if let Some(id) = first_pending_id {
            return Err(id);
        }

        Ok(vec![])
    }

    fn expand_branch(&self, branch: &PlanBranch, candidate: &ActionCandidate) -> Result<Option<PlanBranch>, usize> {
        let mut new_chain = branch.action_chain.clone();
        new_chain.insert(0, candidate.action_idx as i64);

        let action = &self.ctx.actions[candidate.action_idx];
        
        let mut current_agent = self.ctx.initial_agent.clone();
        let mut current_world = self.ctx.initial_world.clone();
        let mut total_cost = 0.0;
        
        let mut new_bindings = branch.action_bindings.clone();
        for binding in &mut new_bindings {
            binding.0 += 1;
        }
        
        if !candidate.satisfied_requirement_indices.is_empty() {
            for &req_idx in &candidate.satisfied_requirement_indices {
                let (consumer_pos, req) = &branch.open_requirements[req_idx];
                let new_consumer_pos = consumer_pos + 1;

                for prov in &action.provisions {
                    if crate::requirement::provision_satisfies_requirement(prov, req, Some(&self.ctx.initial_world)) {
                        if let Some(binding) = extract_binding(prov, req) {
                            if matches!(prov, ProvisionSpec::FactWildcard { .. }) {
                                new_bindings.push((0, binding.0.clone(), binding.1.clone()));
                            }
                            new_bindings.push((new_consumer_pos as i64, binding.0, binding.1));
                        }
                    }
                }
            }
        }

        let mut current_provisions = self.ctx.initial_provisions.clone();

        for (pos, &act_idx) in new_chain.iter().enumerate() {
            let action = &self.ctx.actions[act_idx as usize];
            let is_grounded = crate::requirement::requirements_satisfied(&action.requirements, &current_provisions);

            let mut sim_agent = current_agent.clone();
            let mut sim_world = current_world.clone();

            if pos == 0 && !is_grounded {
                apply_requirements_to_snapshots(&action.requirements, &mut sim_agent, &mut sim_world);
            }

            match simulation::simulate_action(
                action,
                pos,
                &new_bindings,
                &sim_agent,
                &sim_world,
                current_provisions.clone(),
                &*self.ctx,
            ) {
                simulation::SimulationStepResult::Ready(res) => {
                    current_agent = res.agent;
                    current_world = res.world;
                    total_cost += res.cost;
                }
                simulation::SimulationStepResult::Pending(id) => return Err(id),
                _ => {
                    log_debug!("Ripple: Action {} at pos {} failed simulation - discarding branch", action.name, pos);
                    return Ok(None);
                }
            }
            
            current_provisions.extend(action.provisions.clone());
        }

        for req in &action.requirements {
            if crate::requirement::requirement_satisfied(req, &self.ctx.initial_provisions) {
                for prov in &self.ctx.initial_provisions {
                    if crate::requirement::provision_satisfies_requirement(prov, req, Some(&self.ctx.initial_world)) {
                        if let Some(binding) = extract_binding(prov, req) {
                            new_bindings.push((0, binding.0, binding.1));
                            break;
                        }
                    }
                }
            }
        }

        let mut new_branch = branch.clone();
        new_branch.action_chain = new_chain;
        new_branch.action_bindings = new_bindings;
        new_branch.final_state_agent = current_agent;
        new_branch.final_state_world = current_world;
        new_branch.bound_provisions = current_provisions;
        new_branch.cost = total_cost;

        let mut reqs = Vec::new();
        let satisfied_indices = &candidate.satisfied_requirement_indices;
        for (i, (consumer_pos, req)) in branch.open_requirements.iter().enumerate() {
            if !satisfied_indices.contains(&i) {
                reqs.push((consumer_pos + 1, req.clone()));
            }
        }
        
        for req in &action.requirements {
            if !crate::requirement::requirement_satisfied(req, &self.ctx.initial_provisions) {
                reqs.push((0, req.clone()));
            }
        }
        new_branch.open_requirements = reqs;

        let mut new_preconds = branch.open_preconditions.clone();
        let mut pre_indices = candidate.satisfied_precondition_indices.to_vec();
        pre_indices.sort_unstable();
        for &idx in pre_indices.iter().rev() {
            new_preconds.remove(idx);
        }

        for pre in &action.preconditions {
            match simulation::eval_precondition(
                pre,
                &branch.final_state_agent,
                &branch.final_state_world,
                branch.bound_provisions.clone(),
                vec![],
                &*self.ctx,
            ) {
                simulation::PreconditionResult::Ready(false) => {
                    if !new_preconds.contains(pre) {
                        new_preconds.push(pre.clone());
                    }
                }
                simulation::PreconditionResult::Pending(id) => return Err(id),
                _ => {}
            }
        }
        new_branch.open_preconditions = new_preconds;

        Ok(Some(new_branch))
    }
}

fn apply_requirements_to_snapshots(reqs: &[RequirementSpec], agent: &mut BlackboardSnapshot, _world: &mut BlackboardSnapshot) {
    for req in reqs {
        match req {
            RequirementSpec::BindingEquals { binding_name, value } => {
                // Apply concrete value requirements to agent snapshot.
                // World-level bindings will need a separate mechanism when needed.
                agent.properties.insert(binding_name.clone(), value.clone());
            }
            _ => {}
        }
    }
}

fn extract_binding(prov: &ProvisionSpec, req: &RequirementSpec) -> Option<(String, Vec<VariantSnapshot>)> {
    match (prov, req) {
        (ProvisionSpec::Binding { binding_name, value }, RequirementSpec::BindingExists { binding_name: req_name })
        | (ProvisionSpec::Binding { binding_name, value }, RequirementSpec::BindingEquals { binding_name: req_name, .. })
        | (ProvisionSpec::Binding { binding_name, value }, RequirementSpec::BindingInSet { binding_name: req_name, .. }) => {
            if binding_name == req_name {
                Some((binding_name.clone(), vec![value.clone()]))
            } else {
                None
            }
        }
        (ProvisionSpec::Fact { fact_name, args }, RequirementSpec::Fact { fact_name: r_name, .. }) => {
            if fact_name == r_name {
                Some((fact_name.clone(), args.clone()))
            } else {
                None
            }
        }
        (ProvisionSpec::FactWildcard { fact_name }, RequirementSpec::Fact { fact_name: r_name, args }) => {
            if fact_name == r_name {
                Some((fact_name.clone(), args.clone()))
            } else {
                None
            }
        }
        _ => None,
    }
}
