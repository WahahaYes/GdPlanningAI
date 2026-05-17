use super::expander::PlanBranch;

pub struct SearchNode {
    pub branch: PlanBranch,
    pub depth: usize,
    pub estimated_remaining: f64,
    /// ID of the corresponding node in [`crate::debug_tree::TreeDump`].
    pub tree_node_id: usize,
}

pub trait SearchController {
    fn push_initial(&mut self, node: SearchNode);
    fn pop_next(&mut self) -> Option<SearchNode>;
    fn push_successors(&mut self, successors: Vec<SearchNode>);
    fn frontier_size(&self) -> usize;
}

pub struct DfsController {
    stack: Vec<SearchNode>,
}

impl DfsController {
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }
}

impl SearchController for DfsController {
    fn push_initial(&mut self, node: SearchNode) {
        self.stack.push(node);
    }

    fn pop_next(&mut self) -> Option<SearchNode> {
        self.stack.pop()
    }

    fn push_successors(&mut self, successors: Vec<SearchNode>) {
        for node in successors.into_iter().rev() {
            self.stack.push(node);
        }
    }

    fn frontier_size(&self) -> usize {
        self.stack.len()
    }
}
