use crate::plan_types::GoalSpec;

pub struct GoalCandidate {
    pub goal_index: usize,
    pub skip_if_satisfied: bool,
}

pub trait GoalSelection {
    fn select_goals(&self, goals: &[GoalSpec]) -> Vec<GoalCandidate>;
    fn short_circuit_on_first_valid(&self) -> bool;
    fn is_better_than(&self, reward_a: f64, cost_a: f64, reward_b: f64, cost_b: f64) -> bool;
}

pub struct HighestRewardFirst {
    max_goals: usize,
}

impl HighestRewardFirst {
    pub fn new(max_goals: usize) -> Self {
        Self { max_goals }
    }
}

impl GoalSelection for HighestRewardFirst {
    fn select_goals(&self, goals: &[GoalSpec]) -> Vec<GoalCandidate> {
        let mut indexed: Vec<(usize, &GoalSpec)> = goals.iter().enumerate().collect();
        indexed.sort_by(|(_, a), (_, b)| {
            b.reward
                .partial_cmp(&a.reward)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let limit = if self.max_goals == 0 {
            indexed.len()
        } else {
            self.max_goals.min(indexed.len())
        };

        indexed
            .into_iter()
            .take(limit)
            .map(|(idx, _)| GoalCandidate {
                goal_index: idx,
                skip_if_satisfied: true,
            })
            .collect()
    }

    fn short_circuit_on_first_valid(&self) -> bool {
        true
    }

    fn is_better_than(&self, reward_a: f64, cost_a: f64, reward_b: f64, cost_b: f64) -> bool {
        match reward_a.partial_cmp(&reward_b) {
            Some(std::cmp::Ordering::Greater) => true,
            Some(std::cmp::Ordering::Less) => false,
            _ => cost_a < cost_b,
        }
    }
}
