//! Core planning engine with GDExtension integration.
//!
//! Implements a forward-chaining recursive search algorithm that explores
//! action sequences to find plans achieving specified goals.

use super::action::ActionData;
use super::gdpai_blackboard::GdPAIBlackboard;
use super::goal::GoalData;
use super::precondition::PreconditionHandler;
use crate::plan_tree::{self, PlanResult, PlanTreeNode};
use godot::prelude::*;

/// Forward-chaining GOAP planning engine exposed to GDScript.
///
/// Given a set of [GdPAIBlackboard] states, [code]Action[/code] dictionaries, and
/// [code]Goal[/code] dictionaries, [method build_plan] performs a recursive
/// forward search and returns the lowest-cost action chain that satisfies
/// the highest-reward achievable goal.
///
/// [b]Usage from GDScript:[/b]
/// [codeblock]
/// var engine := RustPlanningEngine.new()
/// engine.set_max_recursion(10)
/// engine.set_log_level(RustPlanningEngine.LOG_LEVEL_DEBUG)
/// var result := engine.build_plan(agent_bb, world_bb, actions, goals)
/// if result["success"]:
///     var chain = result["action_chain"]
/// [/codeblock]
#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct RustPlanningEngine {
    max_recursion: usize,
    #[base]
    base: Base<RefCounted>,
}

#[godot_api]
impl RustPlanningEngine {
    /// Searches for a plan that satisfies the highest-reward achievable goal.
    ///
    /// [param agent_blackboard] and [param world_state] are [GdPAIBlackboard] objects
    /// representing the agent and world at the moment planning begins.
    /// [param actions] and [param goals] are [code]Array[Dictionary][/code] produced
    /// by the GDScript bridge.
    ///
    /// Returns a [Dictionary] with keys:
    /// [br]- [code]success[/code] ([bool]) — whether a plan was found.
    /// [br]- [code]action_chain[/code] ([code]Array[int][/code]) — indices into [param actions].
    /// [br]- [code]total_cost[/code] ([float]) — cumulative action cost of the plan.
    /// [br]- [code]goal_index[/code] ([int]) — index of the satisfied goal, or [code]-1[/code] on failure.
    #[func]
    fn build_plan(
        &mut self,
        agent_blackboard: Gd<GdPAIBlackboard>,
        world_state: Gd<GdPAIBlackboard>,
        actions: Array<VarDictionary>,
        goals: Array<VarDictionary>,
    ) -> VarDictionary {
        let action_data: Vec<ActionData> = actions
            .iter_shared()
            .filter_map(|dict| ActionData::from_dict(&dict))
            .collect();

        let goal_data: Vec<GoalData> = goals
            .iter_shared()
            .enumerate()
            .filter_map(|(idx, dict)| {
                let mut data = GoalData::from_dict(&dict)?;
                data.original_index = idx;
                Some(data)
            })
            .collect();

        let result = self.plan(agent_blackboard, world_state, action_data, goal_data);
        self.result_to_dict(&result)
    }

    /// Sets the maximum search depth. Branches deeper than this are pruned.
    #[func]
    fn set_max_recursion(&mut self, max: i64) {
        self.max_recursion = max as usize;
    }

    /// Returns the current maximum search depth.
    #[func]
    fn get_max_recursion(&self) -> i64 {
        self.max_recursion as i64
    }

    /// Sets the global log verbosity for the planning engine.
    ///
    /// Accepts one of the [code]LOG_LEVEL_*[/code] constants on this class:
    /// [br]- [code]LOG_LEVEL_ERROR[/code] (0) — errors only.
    /// [br]- [code]LOG_LEVEL_WARN[/code]  (1) — warnings and errors.
    /// [br]- [code]LOG_LEVEL_INFO[/code]  (2) — lifecycle messages (default).
    /// [br]- [code]LOG_LEVEL_DEBUG[/code] (3) — per-action trace; very verbose.
    #[func]
    fn set_log_level(&mut self, level: i64) {
        crate::logger::set_log_level(crate::logger::LogLevel::from_u8(level.clamp(0, 3) as u8));
    }

    /// Returns the current global log level as an integer.
    #[func]
    fn get_log_level(&self) -> i64 {
        crate::logger::get_log_level() as i64
    }

    #[constant]
    const LOG_LEVEL_ERROR: i64 = 0;
    #[constant]
    const LOG_LEVEL_WARN: i64 = 1;
    #[constant]
    const LOG_LEVEL_INFO: i64 = 2;
    #[constant]
    const LOG_LEVEL_DEBUG: i64 = 3;
}

impl RustPlanningEngine {
    /// Tries each goal in descending reward order and returns the first successful plan.
    pub fn plan(
        &mut self,
        agent_state: Gd<GdPAIBlackboard>,
        world_state: Gd<GdPAIBlackboard>,
        actions: Vec<ActionData>,
        goals: Vec<GoalData>,
    ) -> PlanResult {
        log_info!(
            "Planning started — {} action(s), {} goal(s)",
            actions.len(),
            goals.len()
        );

        let mut sorted_goals = goals;
        sorted_goals.sort_by(|a, b| {
            b.reward
                .partial_cmp(&a.reward)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for goal in sorted_goals {
            log_info!(
                "Trying goal '{}' (reward {})",
                goal.name,
                goal.reward as f32
            );
            let result = self.try_build_plan_for_goal(
                agent_state.clone(),
                world_state.clone(),
                &actions,
                &goal,
            );

            if result.success {
                log_info!("Goal '{}' satisfied — plan found", goal.name);
                return PlanResult {
                    goal_index: goal.original_index as i64,
                    ..result
                };
            } else {
                log_info!("Goal '{}' not achievable, trying next", goal.name);
            }
        }

        log_info!("Planning failed: no achievable goal found");
        PlanResult::failure()
    }

    /// Attempts to build a plan for a single goal.
    ///
    /// Returns immediately with an empty action chain if the goal is already
    /// satisfied. Otherwise starts a recursive forward search.
    fn try_build_plan_for_goal(
        &mut self,
        agent_state: Gd<GdPAIBlackboard>,
        world_state: Gd<GdPAIBlackboard>,
        actions: &[ActionData],
        goal: &GoalData,
    ) -> PlanResult {
        log_debug!("Checking if goal '{}' is already satisfied", goal.name);
        if self.is_goal_satisfied(&goal.desired_state, &agent_state, &world_state) {
            log_debug!("Goal '{}' already satisfied — zero-cost plan", goal.name);
            return PlanResult {
                success: true,
                action_chain: vec![],
                total_cost: 0.0,
                goal_index: goal.original_index as i64,
            };
        }

        let mut root_node = PlanTreeNode {
            action_index: -1,
            cost: 0.0,
            children: vec![],
        };

        log_debug!("Starting recursive search for goal '{}'", goal.name);
        let success =
            self.build_plan_recursive(&mut root_node, &goal.desired_state, agent_state, world_state, actions, 0);

        if success {
            log_debug!(
                "Solution found for goal '{}', extracting best path",
                goal.name
            );
            let plan = plan_tree::extract_best_plan(&root_node);
            PlanResult {
                success: true,
                action_chain: plan.actions,
                total_cost: plan.cost,
                goal_index: goal.original_index as i64,
            }
        } else {
            log_info!("No plan found for goal '{}'", goal.name);
            PlanResult::failure()
        }
    }

    /// Core recursive search step.
    ///
    /// Iterates over all actions, skipping invalid or infinite-cost ones.
    /// For each action that makes progress toward the node's desired state,
    /// applies its effect on a cloned blackboard pair and recurses. Returns
    /// `true` if at least one satisfying leaf was found.
    fn build_plan_recursive(
        &mut self,
        node: &mut PlanTreeNode,
        desired_state: &[PreconditionHandler],
        agent_state: Gd<GdPAIBlackboard>,
        world_state: Gd<GdPAIBlackboard>,
        actions: &[ActionData],
        recursion_level: usize,
    ) -> bool {
        if recursion_level > self.max_recursion {
            log_warn!(
                "Search depth limit ({}) reached; pruning branch",
                recursion_level
            );
            return false;
        }

        let mut has_solution = false;

        log_debug!(
            "Level {}: checking {} available action(s)",
            recursion_level,
            actions.len()
        );

        for (idx, action) in actions.iter().enumerate() {
            log_debug!(
                "Level {}: evaluating action [{}] '{}'",
                recursion_level,
                idx,
                action.name
            );

            if !action.is_valid(&agent_state, &world_state) {
                log_debug!(
                    "Level {}: action [{}] '{}' failed validity checks",
                    recursion_level,
                    idx,
                    action.name
                );
                continue;
            }

            let mut sim_agent = agent_state.bind().clone_for_simulation();
            let mut sim_world = world_state.bind().clone_for_simulation();

            let cost = action.get_cost(&sim_agent, &sim_world);
            if cost == f64::INFINITY {
                log_debug!(
                    "Level {}: action [{}] '{}' returned infinite cost, skipping",
                    recursion_level,
                    idx,
                    action.name
                );
                continue;
            }

            log_debug!(
                "Level {}: action [{}] '{}' valid, cost = {}",
                recursion_level,
                idx,
                action.name,
                cost as f32
            );

            action.apply_effect(&mut sim_agent, &mut sim_world);

            let should_use_action =
                self.check_progress_toward_goal(desired_state, &sim_agent, &sim_world);

            if should_use_action {
                log_debug!(
                    "Level {}: action [{}] '{}' makes progress toward goal",
                    recursion_level,
                    idx,
                    action.name
                );

                if self.is_goal_satisfied(desired_state, &sim_agent, &sim_world) {
                    log_debug!(
                        "Level {}: action [{}] '{}' fully satisfies goal — leaf added",
                        recursion_level,
                        idx,
                        action.name
                    );

                    let next_node = PlanTreeNode {
                        action_index: idx as i64,
                        cost,
                        children: vec![],
                    };
                    node.children.push(next_node);
                    has_solution = true;
                    continue;
                }

                let mut next_node = PlanTreeNode {
                    action_index: idx as i64,
                    cost,
                    children: vec![],
                };

                // Propagate the action's preconditions as additional constraints
                // the sub-plan must satisfy before this action can be used.
                let mut child_desired = desired_state.to_vec();
                child_desired.extend(action.preconditions.clone());

                if self.build_plan_recursive(
                    &mut next_node,
                    &child_desired,
                    sim_agent,
                    sim_world,
                    actions,
                    recursion_level + 1,
                ) {
                    log_debug!(
                        "Level {}: valid sub-plan found through action [{}] '{}'",
                        recursion_level,
                        idx,
                        action.name
                    );
                    node.children.push(next_node);
                    has_solution = true;
                }
            } else {
                log_debug!(
                    "Level {}: action [{}] '{}' makes no progress, skipping",
                    recursion_level,
                    idx,
                    action.name
                );
            }
        }

        has_solution
    }

    /// Returns `true` if the post-effect state satisfies at least one of the
    /// goal's preconditions, indicating the action moved closer to the goal.
    fn check_progress_toward_goal(
        &self,
        preconditions: &[PreconditionHandler],
        agent_state: &Gd<GdPAIBlackboard>,
        world_state: &Gd<GdPAIBlackboard>,
    ) -> bool {
        for precond in preconditions {
            if precond.evaluate(agent_state, world_state) {
                return true;
            }
        }
        false
    }

    /// Returns `true` if every goal precondition is satisfied in the given state.
    fn is_goal_satisfied(
        &self,
        preconditions: &[PreconditionHandler],
        agent_state: &Gd<GdPAIBlackboard>,
        world_state: &Gd<GdPAIBlackboard>,
    ) -> bool {
        preconditions
            .iter()
            .all(|p| p.evaluate(agent_state, world_state))
    }

    /// Serialises a [`PlanResult`] into the [`Dictionary`] format returned by [`build_plan`](Self::build_plan).
    fn result_to_dict(&self, result: &PlanResult) -> VarDictionary {
        let mut dict = VarDictionary::new();

        dict.set("success", result.success);
        dict.set("total_cost", result.total_cost);
        dict.set("goal_index", result.goal_index);

        let mut action_chain = Array::<Variant>::new();
        for action_index in &result.action_chain {
            action_chain.push(&action_index.to_variant());
        }
        dict.set("action_chain", action_chain.to_variant());

        dict
    }
}

#[godot_api]
impl IRefCounted for RustPlanningEngine {
    fn init(_base: Base<RefCounted>) -> Self {
        Self {
            max_recursion: 100,
            base: _base,
        }
    }
}

