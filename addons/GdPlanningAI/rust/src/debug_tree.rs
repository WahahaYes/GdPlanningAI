//! Search tree recorded for debugging when the log level is [`LogLevel::Debug`]; exposed as
//! `get_debug_tree` on the scheduler. The search never reads it back.
//!
//! Nodes live in a flat `Vec` and are addressed by `usize` id, which suits the iterative search
//! loop.

use crate::logger::LogLevel;

/// One goal the planner tried to satisfy.
#[derive(Clone, Debug)]
pub struct GoalAttempt {
    /// Goal display name.
    pub goal_name: String,
    /// Reward of the goal.
    pub goal_reward: f64,
    /// The goal's desired state, rendered as text.
    pub goal_preconditions: Vec<String>,
    /// ID of the root node of this goal's search tree.
    pub root_id: usize,
    /// Whether a plan was found.
    pub success: bool,
    /// Names of the plan's actions in execution order; empty on failure.
    pub plan_actions: Vec<String>,
    /// Cost of the plan.
    pub plan_cost: f64,
}

/// A single node in the search tree.
#[derive(Clone, Debug)]
pub struct TreeNode {
    /// `None` for the root node of a goal attempt.
    pub action_name: Option<String>,
    /// Cost this node's action added to the branch (its discovery cost).
    pub estimated_cost: f64,
    /// Total cost of the branch at this node.
    pub accumulated_cost: f64,
    /// Preconditions still unsatisfied after this node, rendered as text.
    pub open_preconditions: Vec<String>,
    /// Requirements still unsatisfied after this node, rendered as text.
    pub open_requirements: Vec<String>,
    /// Open preconditions this node's action satisfied.
    pub satisfied_preconditions: Vec<String>,
    /// Open requirements this node's action satisfied.
    pub satisfied_requirements: Vec<String>,
    /// What eventually happened to the node.
    pub outcome: NodeOutcome,
    /// Steps of forward verification that concern this node's branch.
    pub forward_validation: Vec<FwdStep>,
    /// IDs of child nodes (indexes into TreeDump.nodes).
    pub children: Vec<usize>,
}

/// What happened at this search node.
#[derive(Clone, Debug)]
pub enum NodeOutcome {
    /// Node was expanded — children contain the results. The default for new nodes.
    Expanded,
    /// Pruned (too many open requirements, or forward verification failed or left needs open).
    Pruned {
        /// Short explanation shown in the tree.
        reason: String,
    },
    /// Branch completed — forward validation ran.
    Complete {
        /// Number of actions in the verified chain.
        chain_len: usize,
        /// Total cost of the chain.
        total_cost: f64,
        /// Whether forward validation succeeded.
        fwd_ok: bool,
    },
}

/// One step in forward validation of a completed chain.
#[derive(Clone, Debug)]
pub struct FwdStep {
    /// The action being validated.
    pub action_name: String,
    /// Kind of check, e.g. `requirement`.
    pub step: String,
    /// Human-readable detail, e.g. the requirement that failed.
    pub detail: String,
    /// Whether the check passed.
    pub ok: bool,
}

/// Configuration for creating a child node.
#[derive(Debug, Default)]
pub struct ChildNodeConfig<'a> {
    /// Name of the action the child inserts.
    pub action_name: &'a str,
    /// Cost the action adds.
    pub estimated_cost: f64,
    /// Total branch cost including that action.
    pub accumulated_cost: f64,
    /// Open preconditions after the insertion, rendered as text.
    pub open_pre: &'a [String],
    /// Open requirements after the insertion, rendered as text.
    pub open_req: &'a [String],
    /// Open preconditions the action satisfied.
    pub satisfied_pre: &'a [String],
    /// Open requirements the action satisfied.
    pub satisfied_req: &'a [String],
}

/// Debug tree builder. When disabled, every method returns immediately (ids are 0), so the
/// engine calls them unconditionally. One dump covers all goals of a run.
pub struct TreeDump {
    enabled: bool,
    /// All tree nodes, indexed by their opaque ID.
    nodes: Vec<TreeNode>,
    /// Per-goal metadata (root_id indexes into `nodes`).
    goal_attempts: Vec<GoalAttempt>,
    start_time: std::time::Instant,
    /// Nodes ever allocated, including when disabled. Backs [`TreeDump::summary`]
    /// so the Info-level one-liner can report branch counts without recording.
    branch_count: usize,
}

impl TreeDump {
    /// Create a TreeDump that is enabled only when the log level is Debug or higher.
    /// The level is sampled once, here; changing it later does not affect this dump.
    pub fn new() -> Self {
        let enabled = crate::logger::get_log_level() >= LogLevel::Debug;
        Self::with_enabled(enabled)
    }

    /// Create a TreeDump that is always enabled, regardless of log level.
    pub fn new_forced() -> Self {
        Self::with_enabled(true)
    }

    fn with_enabled(enabled: bool) -> Self {
        Self {
            enabled,
            nodes: Vec::new(),
            goal_attempts: Vec::new(),
            start_time: std::time::Instant::now(),
            branch_count: 0,
        }
    }
}

impl Default for TreeDump {
    fn default() -> Self {
        Self::new()
    }
}

impl TreeDump {
    /// Returns true if tree recording is active.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Milliseconds since this dump was created.
    pub fn elapsed_ms(&self) -> f64 {
        self.start_time.elapsed().as_secs_f64() * 1000.0
    }

    /// One-line search summary for Info-level logging. Works even when the
    /// full tree is disabled so the default log level still reports cost.
    pub fn summary(&self) -> String {
        format!(
            "Branches: {} | Time: {:.1}ms",
            self.branch_count,
            self.elapsed_ms()
        )
    }

    /// Start a new goal attempt. Returns the goal index.
    pub fn begin_goal(&mut self, name: &str, reward: f64, preconditions: &[String]) -> usize {
        if !self.enabled {
            return 0;
        }
        let idx = self.goal_attempts.len();
        self.goal_attempts.push(GoalAttempt {
            goal_name: name.to_string(),
            goal_reward: reward,
            goal_preconditions: preconditions.to_vec(),
            root_id: 0,
            success: false,
            plan_actions: Vec::new(),
            plan_cost: 0.0,
        });
        idx
    }

    /// Finalize the current goal attempt with its result: the plan's action names and cost on
    /// success, empty and 0 on failure.
    pub fn end_goal(&mut self, success: bool, plan_actions: &[String], plan_cost: f64) {
        if !self.enabled {
            return;
        }
        if let Some(ga) = self.goal_attempts.last_mut() {
            ga.success = success;
            ga.plan_actions = plan_actions.to_vec();
            ga.plan_cost = plan_cost;
        }
    }

    /// Creates the root node for the current goal (after `begin_goal`) and returns its id.
    pub fn add_root(&mut self, open_pre: &[String], open_req: &[String]) -> usize {
        self.branch_count += 1;
        if !self.enabled {
            return 0;
        }
        let id = self.nodes.len();
        self.nodes.push(TreeNode {
            action_name: None,
            estimated_cost: 0.0,
            accumulated_cost: 0.0,
            open_preconditions: open_pre.to_vec(),
            open_requirements: open_req.to_vec(),
            satisfied_preconditions: Vec::new(),
            satisfied_requirements: Vec::new(),
            outcome: NodeOutcome::Expanded,
            forward_validation: Vec::new(),
            children: Vec::new(),
        });
        if let Some(ga) = self.goal_attempts.last_mut() {
            ga.root_id = id;
        }
        id
    }

    /// Add a child node under `parent_id`. Returns the new node's ID. Counts towards the
    /// branch summary even when recording is disabled.
    pub fn add_child(&mut self, parent_id: usize, config: ChildNodeConfig<'_>) -> usize {
        self.branch_count += 1;
        if !self.enabled {
            return 0;
        }
        let child_id = self.nodes.len();
        self.nodes.push(TreeNode {
            action_name: Some(config.action_name.to_string()),
            estimated_cost: config.estimated_cost,
            accumulated_cost: config.accumulated_cost,
            open_preconditions: config.open_pre.to_vec(),
            open_requirements: config.open_req.to_vec(),
            satisfied_preconditions: config.satisfied_pre.to_vec(),
            satisfied_requirements: config.satisfied_req.to_vec(),
            outcome: NodeOutcome::Expanded,
            forward_validation: Vec::new(),
            children: Vec::new(),
        });
        if parent_id < self.nodes.len() {
            self.nodes[parent_id].children.push(child_id);
        }
        child_id
    }

    /// Set the outcome of any node.
    pub fn set_outcome(&mut self, node_id: usize, outcome: NodeOutcome) {
        if !self.enabled {
            return;
        }
        if let Some(node) = self.nodes.get_mut(node_id) {
            node.outcome = outcome;
        }
    }

    /// Append a forward-validation step to a node.
    pub fn add_fwd_step(
        &mut self,
        node_id: usize,
        action_name: &str,
        step: &str,
        detail: &str,
        ok: bool,
    ) {
        if !self.enabled {
            return;
        }
        if let Some(node) = self.nodes.get_mut(node_id) {
            node.forward_validation.push(FwdStep {
                action_name: action_name.to_string(),
                step: step.to_string(),
                detail: detail.to_string(),
                ok,
            });
        }
    }

    /// Formats the tree, one section per goal. Empty when recording is off.
    pub fn format(&self) -> String {
        if !self.enabled || self.goal_attempts.is_empty() {
            return String::new();
        }

        let mut output = String::new();
        output.push_str("\n========== PLANNER SEARCH TREE ==========\n");
        for ga in &self.goal_attempts {
            output.push_str(&format!(
                "Goal: '{}' (reward={:.1}) pre=[{}]\n",
                ga.goal_name,
                ga.goal_reward,
                ga.goal_preconditions.join(", ")
            ));
            if ga.root_id < self.nodes.len() {
                format_node(&mut output, self, ga.root_id, 1, &mut vec![false]);
            } else {
                output.push_str("  └── (no search)\n");
            }
            let status = if ga.success { "SUCCESS" } else { "FAILURE" };
            if ga.success && !ga.plan_actions.is_empty() {
                output.push_str(&format!(
                    "  RESULT: {} — [{}] cost={:.2}\n",
                    status,
                    ga.plan_actions.join(" → "),
                    ga.plan_cost
                ));
            } else if ga.success {
                output.push_str(&format!("  RESULT: {} — (empty plan)\n", status));
            } else {
                output.push_str(&format!("  RESULT: {} — no plan found\n", status));
            }
        }
        output.push_str(&self.summary());
        output.push('\n');
        output.push_str("==========================================\n");
        output
    }
}

/// Appends `node_id` and its subtree to `output`. `has_more[d]` says whether the ancestor at
/// depth `d` has later siblings (draw `│` or a blank).
fn format_node(
    output: &mut String,
    tree: &TreeDump,
    node_id: usize,
    depth: usize,
    has_more: &mut Vec<bool>,
) {
    let node = &tree.nodes[node_id];

    let indent: String = (0..depth.saturating_sub(1))
        .map(|d| if has_more[d] { "│   " } else { "    " })
        .collect();
    let branch = if depth == 0 {
        ""
    } else if has_more[depth - 1] {
        "├── "
    } else {
        "└── "
    };
    let prefix = format!("{}{}", indent, branch);

    match &node.outcome {
        NodeOutcome::Expanded | NodeOutcome::Complete { .. } => {
            let label = if let Some(ref name) = node.action_name {
                format!(
                    "Try '{}' (cost=+{:.2}, total={:.2})",
                    name, node.estimated_cost, node.accumulated_cost
                )
            } else {
                "ROOT".to_string()
            };
            output.push_str(&format!("{}{}\n", prefix, label));
            if !node.satisfied_preconditions.is_empty() || !node.satisfied_requirements.is_empty() {
                output.push_str(&format!(
                    "{}  satisfies_pre=[{}] satisfies_req=[{}]\n",
                    indent,
                    node.satisfied_preconditions.join(", "),
                    node.satisfied_requirements.join(", ")
                ));
            }
            if !node.open_preconditions.is_empty() || !node.open_requirements.is_empty() {
                output.push_str(&format!(
                    "{}  open_pre=[{}] open_req=[{}]\n",
                    indent,
                    node.open_preconditions.join(", "),
                    node.open_requirements.join(", ")
                ));
            }
        }
        NodeOutcome::Pruned { reason } => {
            let name = node.action_name.as_deref().unwrap_or("?");
            output.push_str(&format!("{}'{}' PRUNED: {}\n", prefix, name, reason));
        }
    }

    for step in &node.forward_validation {
        let status = if step.ok { "OK" } else { "FAIL" };
        output.push_str(&format!(
            "{}  FWD [{}] '{}' {}: {}\n",
            indent, status, step.action_name, step.step, step.detail
        ));
    }

    let n = node.children.len();
    has_more.push(false);
    for (i, &child_id) in node.children.iter().enumerate() {
        has_more[depth] = i + 1 < n;
        format_node(output, tree, child_id, depth + 1, has_more);
    }
    has_more.pop();
}
