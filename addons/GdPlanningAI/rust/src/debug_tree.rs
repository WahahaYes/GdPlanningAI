//! Structured debug tree for planner search visualization.
//!
//! When log level is [`LogLevel::Debug`], the planner records every branch
//! explored during backward search and dumps a formatted tree at completion.
//! This makes it possible to see the full search space: which branches were
//! tried, why they were pruned, and which path succeeded.

use crate::logger::LogLevel;

/// A single event in the search tree.
#[derive(Clone)]
pub enum TreeEvent {
    /// Entering a new search node for an action.
    EnterAction {
        action_name: String,
        cost: f64,
        accumulated_cost: f64,
        open_preconditions: Vec<String>,
        open_requirements: Vec<String>,
    },
    /// Branch was pruned (cost too high, max depth, etc.).
    Prune {
        reason: String,
    },
    /// No candidate actions found at this depth.
    NoCandidates,
    /// A candidate was skipped (e.g., couldn't resolve pending effects).
    CandidateSkipped {
        action_name: String,
        reason: String,
    },
    /// Branch completed successfully.
    Complete {
        chain_len: usize,
        total_cost: f64,
    },
    /// Forward validation failed for a completed branch.
    ForwardValidationFailed {
        failed_action: String,
        failed_step: String,
        detail: String,
    },
    /// A step during forward validation of a completed chain.
    ForwardValidationStep {
        action_name: String,
        step: String,
        detail: String,
        ok: bool,
    },
    /// Root node — planning start.
    Root {
        goal_name: String,
        goal_reward: f64,
        goal_preconditions: Vec<String>,
    },
    /// An action was excluded from candidates with a reason.
    ActionExcluded {
        action_name: String,
        reason: String,
    },
    /// Goal was already satisfied before planning.
    GoalAlreadySatisfied {
        goal_name: String,
    },
    /// Planning result.
    Result {
        success: bool,
        message: String,
    },
}

/// A node in the debug tree.
#[derive(Clone)]
pub struct TreeEntry {
    pub depth: usize,
    pub event: TreeEvent,
}

/// Accumulates tree entries during planning and formats them for output.
pub struct TreeDump {
    entries: Vec<TreeEntry>,
    enabled: bool,
}

impl TreeDump {
    pub fn new() -> Self {
        let enabled = crate::logger::get_log_level() >= LogLevel::Debug;
        Self {
            entries: Vec::new(),
            enabled,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn record(&mut self, depth: usize, event: TreeEvent) {
        if self.enabled {
            self.entries.push(TreeEntry { depth, event });
        }
    }

    /// Format the recorded tree as a human-readable string.
    pub fn format(&self) -> String {
        if self.entries.is_empty() {
            return String::new();
        }

        let mut output = String::new();
        output.push_str("\n========== PLANNER SEARCH TREE ==========\n");

        // Track which depth levels have "continuation" lines for tree drawing
        let mut depth_has_more: Vec<bool> = vec![false; 64];

        // Pre-compute: for each entry, does a sibling follow at the same depth?
        // We need to know this to draw "├──" vs "└──".
        let _total = self.entries.len();
        for (i, entry) in self.entries.iter().enumerate() {
            let has_sibling = self.entries[i + 1..]
                .iter()
                .any(|e| e.depth == entry.depth);

            // Update depth_has_more for this depth
            if entry.depth < depth_has_more.len() {
                depth_has_more[entry.depth] = has_sibling;
                // Clear deeper levels
                for d in entry.depth + 1..depth_has_more.len() {
                    depth_has_more[d] = false;
                }
            }

            // Build the tree prefix
            let prefix = build_tree_prefix(entry.depth, &depth_has_more, !has_sibling);

            match &entry.event {
                TreeEvent::Root {
                    goal_name,
                    goal_reward,
                    goal_preconditions,
                } => {
                    output.push_str(&format!(
                        "{}ROOT: goal='{}' (reward={:.1}) preconditions=[{}]\n",
                        prefix,
                        goal_name,
                        goal_reward,
                        goal_preconditions.join(", ")
                    ));
                }
                TreeEvent::EnterAction {
                    action_name,
                    cost,
                    accumulated_cost,
                    open_preconditions,
                    open_requirements,
                } => {
                    let precond_str = if open_preconditions.is_empty() {
                        "[]".to_string()
                    } else {
                        format!("[{}]", open_preconditions.join(", "))
                    };
                    let req_str = if open_requirements.is_empty() {
                        "[]".to_string()
                    } else {
                        format!("[{}]", open_requirements.join(", "))
                    };
                    output.push_str(&format!(
                        "{}Try '{}' (cost=+{:.2}, total={:.2}) open_pre={} open_req={}\n",
                        prefix, action_name, cost, accumulated_cost, precond_str, req_str
                    ));
                }
                TreeEvent::Prune { reason } => {
                    output.push_str(&format!("{}PRUNE: {}\n", prefix, reason));
                }
                TreeEvent::NoCandidates => {
                    output.push_str(&format!("{}DEAD END: no candidates\n", prefix));
                }
                TreeEvent::CandidateSkipped {
                    action_name,
                    reason,
                } => {
                    output.push_str(&format!(
                        "{}SKIP '{}': {}\n",
                        prefix, action_name, reason
                    ));
                }
                TreeEvent::Complete {
                    chain_len,
                    total_cost,
                } => {
                    output.push_str(&format!(
                        "{}COMPLETE: {} actions, cost={:.2}\n",
                        prefix, chain_len, total_cost
                    ));
                }
                TreeEvent::ForwardValidationStep {
                    action_name,
                    step,
                    detail,
                    ok,
                } => {
                    let status = if *ok { "OK" } else { "FAIL" };
                    output.push_str(&format!(
                        "{}FWD [{}] '{}' {}: {}\n",
                        prefix, status, action_name, step, detail
                    ));
                }
                TreeEvent::ForwardValidationFailed {
                    failed_action,
                    failed_step,
                    detail,
                } => {
                    output.push_str(&format!(
                        "{}FWD VALIDATION FAILED: '{}' {} — {}\n",
                        prefix, failed_action, failed_step, detail
                    ));
                }
                TreeEvent::ActionExcluded {
                    action_name,
                    reason,
                } => {
                    output.push_str(&format!(
                        "{}EXCLUDE '{}': {}\n",
                        prefix, action_name, reason
                    ));
                }
                TreeEvent::GoalAlreadySatisfied { goal_name } => {
                    output.push_str(&format!(
                        "{}GOAL SATISFIED: '{}' already met\n",
                        prefix, goal_name
                    ));
                }
                TreeEvent::Result { success, message } => {
                    let status = if *success { "SUCCESS" } else { "FAILURE" };
                    output.push_str(&format!(
                        "{}RESULT: {} — {}\n",
                        prefix, status, message
                    ));
                }
            }
        }

        output.push_str("==========================================\n");
        output
    }
}

/// Build the tree-drawing prefix for a given depth.
/// `depth_has_more[d]` is true if there are more siblings at depth d.
/// `is_last` is true if this is the last child at this depth.
fn build_tree_prefix(depth: usize, depth_has_more: &[bool], is_last: bool) -> String {
    if depth == 0 {
        return String::new();
    }

    let mut prefix = String::new();
    for d in 0..depth {
        if d == depth - 1 {
            // This is the connector to the current node
            if is_last {
                prefix.push_str("└── ");
            } else {
                prefix.push_str("├── ");
            }
        } else if depth_has_more[d] {
            prefix.push_str("│   ");
        } else {
            prefix.push_str("    ");
        }
    }
    prefix
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_tree_produces_empty_output() {
        let dump = TreeDump::new();
        assert!(dump.format().is_empty() || !dump.is_enabled());
    }

    #[test]
    fn tree_prefix_root_is_empty() {
        let has_more = vec![false; 64];
        assert_eq!(build_tree_prefix(0, &has_more, false), "");
    }

    #[test]
    fn tree_prefix_depth1_last() {
        let has_more = vec![false; 64];
        assert_eq!(build_tree_prefix(1, &has_more, true), "└── ");
    }

    #[test]
    fn tree_prefix_depth1_not_last() {
        let has_more = vec![false; 64];
        assert_eq!(build_tree_prefix(1, &has_more, false), "├── ");
    }

    #[test]
    fn tree_prefix_depth2_with_continuation() {
        let mut has_more = vec![false; 64];
        has_more[0] = true;
        assert_eq!(build_tree_prefix(2, &has_more, true), "│   └── ");
    }
}
