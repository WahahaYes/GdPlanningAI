//! Shared plan-tree types used during planning.

/// Internal plan tree node for tracking search paths.
#[derive(Clone, Debug)]
pub struct PlanTreeNode {
    pub action_index: i64,
    pub cost: f64,
    pub children: Vec<PlanTreeNode>,
}

/// Result of the planning algorithm.
#[derive(Clone, Debug)]
pub struct PlanResult {
    pub success: bool,
    pub action_chain: Vec<i64>,
    pub total_cost: f64,
    /// Index into the original goals array passed from GDScript; -1 on failure
    pub goal_index: i64,
}

impl PlanResult {
    pub fn failure() -> Self {
        Self {
            success: false,
            action_chain: vec![],
            total_cost: f64::INFINITY,
            goal_index: -1,
        }
    }
}

pub struct ExtractedPlan {
    pub actions: Vec<i64>,
    pub cost: f64,
}

/// Traverses the completed plan tree and returns the path with the lowest total cost.
pub fn extract_best_plan(root: &PlanTreeNode) -> ExtractedPlan {
    let mut best_path: Vec<i64> = vec![];
    let mut best_cost = f64::INFINITY;

    find_lowest_cost_path(root, 0.0, vec![], &mut best_path, &mut best_cost);

    ExtractedPlan {
        actions: best_path,
        cost: best_cost,
    }
}

/// Recursive depth-first traversal that updates `best_path` and `best_cost`
/// whenever a leaf is reached with a lower cumulative cost.
fn find_lowest_cost_path(
    node: &PlanTreeNode,
    current_cost: f64,
    current_path: Vec<i64>,
    best_path: &mut Vec<i64>,
    best_cost: &mut f64,
) {
    let new_cost = current_cost + node.cost;
    let mut new_path = current_path.clone();

    if node.action_index >= 0 {
        new_path.push(node.action_index);
    }

    if node.children.is_empty() {
        if new_cost < *best_cost {
            *best_path = new_path;
            *best_cost = new_cost;
        }
        return;
    }

    for child in &node.children {
        find_lowest_cost_path(child, new_cost, new_path.clone(), best_path, best_cost);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(action_index: i64, cost: f64) -> PlanTreeNode {
        PlanTreeNode {
            action_index,
            cost,
            children: vec![],
        }
    }

    fn node(action_index: i64, cost: f64, children: Vec<PlanTreeNode>) -> PlanTreeNode {
        PlanTreeNode {
            action_index,
            cost,
            children,
        }
    }

    fn root(children: Vec<PlanTreeNode>) -> PlanTreeNode {
        PlanTreeNode {
            action_index: -1,
            cost: 0.0,
            children,
        }
    }

    #[test]
    fn single_action_plan_returned() {
        let tree = root(vec![leaf(0, 5.0)]);
        let plan = extract_best_plan(&tree);
        assert_eq!(plan.actions, vec![0]);
        assert_eq!(plan.cost, 5.0);
    }

    #[test]
    fn picks_lowest_cost_single_step_branch() {
        let tree = root(vec![leaf(0, 20.0), leaf(1, 8.0)]);
        let plan = extract_best_plan(&tree);
        assert_eq!(plan.actions, vec![1]);
        assert_eq!(plan.cost, 8.0);
    }

    #[test]
    fn multi_step_chain_cumulates_cost() {
        let tree = root(vec![node(0, 3.0, vec![leaf(1, 7.0)])]);
        let plan = extract_best_plan(&tree);
        assert_eq!(plan.actions, vec![0, 1]);
        assert_eq!(plan.cost, 10.0);
    }

    #[test]
    fn picks_cheapest_multi_step_path() {
        let tree = root(vec![
            node(0, 5.0, vec![leaf(1, 10.0)]),
            node(2, 4.0, vec![leaf(3, 3.0)]),
        ]);
        let plan = extract_best_plan(&tree);
        assert_eq!(plan.actions, vec![2, 3]);
        assert_eq!(plan.cost, 7.0);
    }

    #[test]
    fn empty_root_returns_zero_cost_empty_plan() {
        let tree = root(vec![]);
        let plan = extract_best_plan(&tree);
        assert_eq!(plan.actions, Vec::<i64>::new());
        assert_eq!(plan.cost, 0.0);
    }
}
