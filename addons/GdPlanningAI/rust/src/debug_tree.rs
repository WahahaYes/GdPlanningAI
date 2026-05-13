//! Structured debug tree for planner search visualization.
//!
//! When log level is [`LogLevel::Debug`], the planner builds a full search tree
//! during backward search. The tree can be formatted as human-readable text or
//! serialized for Godot-side debug tooling.

use crate::logger::LogLevel;

// ── Tree data structures ───────────────────────────────────────────

/// Top-level container for a planning run.
#[derive(Clone, Debug)]
pub struct SearchTree {
    pub goal_attempts: Vec<GoalAttempt>,
    pub elapsed_ms: f64,
    pub branches_explored: usize,
}

/// One goal the planner tried to satisfy.
#[derive(Clone, Debug)]
pub struct GoalAttempt {
    pub goal_name: String,
    pub goal_reward: f64,
    pub goal_preconditions: Vec<String>,
    pub already_satisfied: bool,
    pub root: Option<SearchNode>,
    pub success: bool,
    pub plan_actions: Vec<String>,
    pub plan_cost: f64,
}

/// A single node in the search tree.
#[derive(Clone, Debug)]
pub struct SearchNode {
    /// `None` for the root node of a goal attempt.
    pub action_name: Option<String>,
    pub estimated_cost: f64,
    pub accumulated_cost: f64,
    pub open_preconditions: Vec<String>,
    pub open_requirements: Vec<String>,
    pub excluded_actions: Vec<ExcludedAction>,
    pub outcome: NodeOutcome,
    pub forward_validation: Vec<FwdStep>,
    pub children: Vec<SearchNode>,
}

/// An action that was considered but excluded from candidates.
#[derive(Clone, Debug)]
pub struct ExcludedAction {
    pub action_name: String,
    pub reason: String,
}

/// What happened at this search node.
#[derive(Clone, Debug)]
pub enum NodeOutcome {
    /// Node was expanded — children contain the results.
    Expanded,
    /// Pruned before expansion (cost, depth, cancellation).
    Pruned { reason: String },
    /// No candidates could satisfy the open needs.
    DeadEnd,
    /// Candidate was skipped (e.g. pending effects couldn't resolve).
    Skipped { reason: String },
    /// Branch completed — forward validation ran.
    Complete {
        chain_len: usize,
        total_cost: f64,
        fwd_ok: bool,
    },
}

/// One step in forward validation of a completed chain.
#[derive(Clone, Debug)]
pub struct FwdStep {
    pub action_name: String,
    pub step: String,
    pub detail: String,
    pub ok: bool,
}

// ── Stack-based tree builder ───────────────────────────────────────

/// Builds a [`SearchTree`] during planning via push/pop operations.
pub struct TreeDump {
    enabled: bool,
    tree: SearchTree,
    /// Stack of node indices navigating to the current cursor position.
    node_stack: Vec<usize>,
    /// Owning reference to the current node's children list (index into node_stack).
    start_time: std::time::Instant,
}

impl TreeDump {
    pub fn new() -> Self {
        let enabled = crate::logger::get_log_level() >= LogLevel::Debug;
        Self {
            enabled,
            tree: SearchTree { goal_attempts: Vec::new(), elapsed_ms: 0.0, branches_explored: 0 },
            node_stack: Vec::new(),
            start_time: std::time::Instant::now(),
        }
    }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn begin_goal(&mut self, name: &str, reward: f64, preconditions: &[String]) {
        if !self.enabled { return; }
        self.tree.goal_attempts.push(GoalAttempt {
            goal_name: name.to_string(), goal_reward: reward,
            goal_preconditions: preconditions.to_vec(), already_satisfied: false,
            root: None, success: false, plan_actions: Vec::new(), plan_cost: 0.0,
        });
    }

    pub fn goal_already_satisfied(&mut self) {
        if !self.enabled { return; }
        if let Some(ga) = self.tree.goal_attempts.last_mut() { ga.already_satisfied = true; ga.success = true; }
    }

    pub fn end_goal(&mut self, success: bool, plan_actions: &[String], plan_cost: f64) {
        if !self.enabled { return; }
        if let Some(ga) = self.tree.goal_attempts.last_mut() {
            ga.success = success; ga.plan_actions = plan_actions.to_vec(); ga.plan_cost = plan_cost;
        }
    }

    pub fn enter_node(&mut self, action_name: Option<&str>, estimated_cost: f64, accumulated_cost: f64, open_pre: &[String], open_req: &[String]) {
        if !self.enabled { return; }
        let node = SearchNode {
            action_name: action_name.map(|s| s.to_string()), estimated_cost, accumulated_cost,
            open_preconditions: open_pre.to_vec(), open_requirements: open_req.to_vec(),
            excluded_actions: Vec::new(), outcome: NodeOutcome::Expanded,
            forward_validation: Vec::new(), children: Vec::new(),
        };
        if self.node_stack.is_empty() {
            self.tree.goal_attempts.last_mut().unwrap().root = Some(node);
        } else {
            let parent = self.current_node_mut();
            parent.children.push(node);
        }
        self.node_stack.push(0);
        self.tree.branches_explored += 1;
    }

    pub fn exit_node(&mut self, outcome: NodeOutcome) {
        if !self.enabled { return; }
        self.current_node_mut().outcome = outcome;
        self.node_stack.pop();
    }

    pub fn exclude_action(&mut self, action_name: &str, reason: &str) {
        if !self.enabled { return; }
        self.current_node_mut().excluded_actions.push(ExcludedAction { action_name: action_name.to_string(), reason: reason.to_string() });
    }

    pub fn add_fwd_step(&mut self, action_name: &str, step: &str, detail: &str, ok: bool) {
        if !self.enabled { return; }
        self.current_node_mut().forward_validation.push(FwdStep { action_name: action_name.to_string(), step: step.to_string(), detail: detail.to_string(), ok });
    }

    pub fn finish(mut self) -> SearchTree {
        self.tree.elapsed_ms = self.start_time.elapsed().as_secs_f64() * 1000.0;
        self.tree
    }

    pub fn format(&self) -> String {
        if !self.enabled || self.tree.goal_attempts.is_empty() { return String::new(); }
        let mut output = String::new();
        output.push_str("\n========== PLANNER SEARCH TREE ==========\n");
        for ga in &self.tree.goal_attempts {
            output.push_str(&format!("Goal: '{}' (reward={:.1}) pre=[{}]\n", ga.goal_name, ga.goal_reward, ga.goal_preconditions.join(", ")));
            if ga.already_satisfied { output.push_str("  └── ALREADY SATISFIED\n"); }
            else if let Some(ref root) = ga.root { format_node(&mut output, root, 1, &mut vec![false; 32]); }
            else { output.push_str("  └── (no search)\n"); }
            let status = if ga.success { "SUCCESS" } else { "FAILURE" };
            if ga.success && !ga.plan_actions.is_empty() {
                output.push_str(&format!("  RESULT: {} — [{}] cost={:.2}\n", status, ga.plan_actions.join(" → "), ga.plan_cost));
            } else if ga.success { output.push_str(&format!("  RESULT: {} — (empty plan)\n", status)); }
            else { output.push_str(&format!("  RESULT: {} — no plan found\n", status)); }
        }
        output.push_str(&format!("Branches: {} | Time: {:.1}ms\n", self.tree.branches_explored, self.tree.elapsed_ms));
        output.push_str("==========================================\n");
        output
    }

    fn current_node_mut(&mut self) -> &mut SearchNode {
        let ga = self.tree.goal_attempts.last_mut().unwrap();
        let root = ga.root.as_mut().unwrap();
        let mut cursor = root;
        for _ in 1..self.node_stack.len() { cursor = cursor.children.last_mut().unwrap(); }
        cursor
    }
}

fn format_node(output: &mut String, node: &SearchNode, depth: usize, has_more: &mut [bool]) {
    let indent: String = (0..depth - 1).map(|d| if has_more[d] { "│   " } else { "    " }).collect();
    let branch = if depth == 0 { "" } else if has_more[depth - 1] { "├── " } else { "└── " };
    let prefix = format!("{}{}", indent, branch);

    match &node.outcome {
        NodeOutcome::Expanded | NodeOutcome::Complete { .. } => {
            let label = if let Some(ref name) = node.action_name {
                format!("Try '{}' (cost=+{:.2}, total={:.2})", name, node.estimated_cost, node.accumulated_cost)
            } else { "ROOT".to_string() };
            output.push_str(&format!("{}{}\n", prefix, label));
            if !node.open_preconditions.is_empty() || !node.open_requirements.is_empty() {
                output.push_str(&format!("{}  open_pre=[{}] open_req=[{}]\n", indent, node.open_preconditions.join(", "), node.open_requirements.join(", ")));
            }
        }
        NodeOutcome::Pruned { reason } => {
            let name = node.action_name.as_deref().unwrap_or("?");
            output.push_str(&format!("{}'{}' PRUNED: {}\n", prefix, name, reason));
        }
        NodeOutcome::DeadEnd => {
            output.push_str(&format!("{}DEAD END (no candidates)\n", prefix));
        }
        NodeOutcome::Skipped { reason } => {
            let name = node.action_name.as_deref().unwrap_or("?");
            output.push_str(&format!("{}'{}' SKIPPED: {}\n", prefix, name, reason));
        }
    }

    for ex in &node.excluded_actions {
        output.push_str(&format!("{}  EXCLUDE '{}': {}\n", indent, ex.action_name, ex.reason));
    }

    for step in &node.forward_validation {
        let status = if step.ok { "OK" } else { "FAIL" };
        output.push_str(&format!("{}  FWD [{}] '{}' {}: {}\n", indent, status, step.action_name, step.step, step.detail));
    }

    let n = node.children.len();
    for (i, child) in node.children.iter().enumerate() {
        has_more[depth] = i + 1 < n;
        format_node(output, child, depth + 1, has_more);
    }
}
