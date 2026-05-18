use crate::plan_types::*;
use crate::snapshot::{BlackboardSnapshot, VariantSnapshot};
use crate::requirement::{ProvisionSpec, RequirementSpec};
use super::types::{PlanBranch, ActionCandidate};
use super::simulation;
use std::sync::mpsc::Sender;

pub struct SearchContext<'a> {
    pub actions: &'a [ActionSpec],
    pub initial_agent: &'a BlackboardSnapshot,
    pub initial_world: &'a BlackboardSnapshot,
    pub initial_provisions: &'a [ProvisionSpec],
    pub request_tx: &'a Sender<CallbackRequest>,
}

pub struct BranchExpander<'a> {
    pub ctx: &'a SearchContext<'a>,
}

impl<'a> BranchExpander<'a> {
    pub fn expand(&self, branch: &PlanBranch) -> Vec<PlanBranch> {
        let candidates = self.find_candidates(branch);
        let mut successors = Vec::new();

        for candidate in candidates {
            if let Some(new_branch) = self.expand_branch(branch, &candidate) {
                successors.push(new_branch);
            }
        }

        successors
    }

    fn find_candidates(&self, branch: &PlanBranch) -> Vec<ActionCandidate> {
        let mut candidates = Vec::new();
        log_debug!("find_candidates: Checking {} actions against {} preconds and {} requirements", self.ctx.actions.len(), branch.open_preconditions.len(), branch.open_requirements.len());

        for (idx, action) in self.ctx.actions.iter().enumerate() {
            let mut satisfied_preconditions = Vec::new();
            let mut satisfied_requirements = Vec::new();

            // 1. Check Symbolic Requirements
            for (req_idx, (_, req)) in branch.open_requirements.iter().enumerate() {
                for prov in &action.provisions {
                    if crate::requirement::provision_satisfies_requirement(prov, req, Some(self.ctx.initial_world)) {
                        if !satisfied_requirements.contains(&req_idx) {
                            satisfied_requirements.push(req_idx);
                        }
                    }
                }
            }

            // 2. Check Physical Preconditions (Optimistically)
            // Create a hypothetical state where all of THIS action's requirements 
            // AND all current branch requirements are assumed met.
            let mut hypothetical_agent = self.ctx.initial_agent.clone();
            let mut hypothetical_world = self.ctx.initial_world.clone();
            
            // Apply branch requirements
            let branch_reqs: Vec<RequirementSpec> = branch.open_requirements.iter().map(|(_, r)| r.clone()).collect();
            apply_requirements_to_snapshots(&branch_reqs, &mut hypothetical_agent, &mut hypothetical_world);
            
            // Apply this action's requirements
            apply_requirements_to_snapshots(&action.requirements, &mut hypothetical_agent, &mut hypothetical_world);

            for (pre_idx, precond) in branch.open_preconditions.iter().enumerate() {
                // Run optimistic simulation (ignoring unmet requirements)
                log_debug!("Discovery: Starting simulation for action {}", action.name);
                
                let sim_result = simulation::simulate_action(
                    action,
                    0,
                    &[], // No bindings yet during discovery
                    &hypothetical_agent,
                    &hypothetical_world,
                    self.ctx.initial_provisions.to_vec(),
                    self.ctx.request_tx,
                    true, // skip_validity = true (optimistic)
                );

                if let Some(res) = sim_result {
                    log_debug!("Discovery: Simulation success for {}. Agent props: {:?}", action.name, res.agent.properties);
                    
                    // See if the effect satisfied the precondition
                    if simulation::eval_precondition(
                        precond,
                        &res.agent,
                        &res.world,
                        self.ctx.initial_provisions.to_vec(),
                        vec![], // No bindings yet during discovery
                        self.ctx.request_tx,
                    ) {
                        log_debug!("Discovery: Action {} satisfied precond {:?}", action.name, precond);
                        satisfied_preconditions.push(pre_idx);
                    } else {
                        log_debug!("Discovery: Action {} did NOT satisfy precond {:?}", action.name, precond);
                    }
                } else {
                    log_debug!("Discovery: Action {} simulation RETURNED NONE", action.name);
                }
            }

            if !satisfied_preconditions.is_empty() || !satisfied_requirements.is_empty() {
                // Heuristic: Symbolic matches are slightly more "expensive" to prioritize direct grounding
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

        if candidates.is_empty() {
            log_debug!("No candidates found for branch with {} preconds and {} requirements", branch.open_preconditions.len(), branch.open_requirements.len());
        }

        candidates
    }

    fn expand_branch(&self, branch: &PlanBranch, candidate: &ActionCandidate) -> Option<PlanBranch> {
        let mut new_chain = branch.action_chain.clone();
        new_chain.insert(0, candidate.action_idx as i64);

        let action = &self.ctx.actions[candidate.action_idx];

        // 1. Check if the newly prepended action can eventually be grounded.
        // We do a loose check here: if the action has preconditions, can they be met by the initial state
        // OR are we allowed to search deeper to meet them?
        // Actually, the A* search handles this by only considering a branch 'complete' 
        // when open_preconditions are empty.
        
        // 2. Full Forward Simulation (The Ripple)
        let mut current_agent = self.ctx.initial_agent.clone();
        let mut current_world = self.ctx.initial_world.clone();
        let mut total_cost = 0.0;
        
        // We need to handle bindings.
        let mut new_bindings = branch.action_bindings.clone();
        // Shift existing binding indices
        for binding in &mut new_bindings {
            binding.0 += 1;
        }
        
        // Handle discovery-time bindings (for requirements)
        if !candidate.satisfied_requirement_indices.is_empty() {
            for &req_idx in &candidate.satisfied_requirement_indices {
                let (consumer_pos, req) = &branch.open_requirements[req_idx];
                let new_consumer_pos = consumer_pos + 1; // It was pos, now it's pos+1 because we prepended an action

                for prov in &action.provisions {
                    if crate::requirement::provision_satisfies_requirement(prov, req, Some(self.ctx.initial_world)) {
                        if let Some(binding) = extract_binding(prov, req) {
                            new_bindings.push((new_consumer_pos as i64, binding.0, binding.1));
                        }
                    }
                }
            }
        }

        let mut current_provisions = self.ctx.initial_provisions.to_vec();

        for (pos, &act_idx) in new_chain.iter().enumerate() {
            let action = &self.ctx.actions[act_idx as usize];
            
            // Head of the chain (pos 0) is allowed to be ungrounded in backward planning.
            // If it's ungrounded, we simulate it OPTIMISTICALLY so we can see its potential effects.
            let is_grounded = crate::requirement::requirements_satisfied(&action.requirements, &current_provisions);
            let skip_validity = pos == 0 && !is_grounded;

            let mut sim_agent = current_agent.clone();
            let mut sim_world = current_world.clone();

            if skip_validity {
                // Apply optimistic requirements to the state before simulation
                apply_requirements_to_snapshots(&action.requirements, &mut sim_agent, &mut sim_world);
            }

            if let Some(res) = simulation::simulate_action(
                action,
                pos,
                &new_bindings,
                &sim_agent,
                &sim_world,
                current_provisions.clone(),
                self.ctx.request_tx,
                skip_validity, 
            ) {
                current_agent = res.agent;
                current_world = res.world;
                total_cost += res.cost;
            } else if pos > 0 {
                // If a non-head action fails its grounding check, this branch is invalid.
                log_debug!("Ripple failed at pos {} (action: {}) - discarding branch", pos, action.name);
                return None;
            } else {
                log_debug!("Ripple: head action {} simulation failed even with optimistic skip", action.name);
            }
            
            // Collect provisions from THIS action for the NEXT one
            current_provisions.extend(action.provisions.clone());
        }

        // 2. Verify Progress
        // (Handled implicitly because simulate_action returned Some and is_complete will check the goal)

        // 3. Update Needs
        // Handle pre-binding initial provisions to the new action
        for req in &action.requirements {
            if crate::requirement::requirement_satisfied(req, self.ctx.initial_provisions) {
                for prov in self.ctx.initial_provisions {
                    if crate::requirement::provision_satisfies_requirement(prov, req, Some(self.ctx.initial_world)) {
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
        new_branch.cost = total_cost;

        // Update requirements: shift existing and remove satisfied
        let mut reqs = Vec::new();
        let satisfied_indices = &candidate.satisfied_requirement_indices;
        for (i, (consumer_pos, req)) in branch.open_requirements.iter().enumerate() {
            if !satisfied_indices.contains(&i) {
                reqs.push((consumer_pos + 1, req.clone()));
            }
        }
        
        // Add new action's requirements (all starting at pos 0)
        // Only if they are NOT met by InitialState provisions (already handled above)
        for req in &action.requirements {
            if !crate::requirement::requirement_satisfied(req, self.ctx.initial_provisions) {
                reqs.push((0, req.clone()));
            }
        }
        new_branch.open_requirements = reqs;

        // Update Preconditions (The Frontier)
        // Remove satisfied preconditions and add the new action's preconditions
        let mut new_preconds = branch.open_preconditions.clone();
        let mut pre_indices = candidate.satisfied_precondition_indices.to_vec();
        pre_indices.sort_unstable();
        for &idx in pre_indices.iter().rev() {
            new_preconds.remove(idx);
        }

        for pre in &action.preconditions {
            if !simulation::eval_precondition(
                pre,
                self.ctx.initial_agent,
                self.ctx.initial_world,
                self.ctx.initial_provisions.to_vec(),
                vec![], // No bindings for backward grounding check
                self.ctx.request_tx,
            ) {
                if !new_preconds.contains(pre) {
                    new_preconds.push(pre.clone());
                }
            }
        }
        new_branch.open_preconditions = new_preconds;

        Some(new_branch)
    }
}

fn apply_requirements_to_snapshots(reqs: &[RequirementSpec], agent: &mut BlackboardSnapshot, _world: &mut BlackboardSnapshot) {
    for req in reqs {
        match req {
            RequirementSpec::BindingEquals { binding_name, value } => {
                // We only apply concrete value requirements. 
                // We no longer provide dummy values for Existence or Set requirements.
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
