use crate::config::ActivityWeights;

/// Metrics for repository activity
#[derive(Debug, Clone, Default)]
pub struct ActivityMetrics {
    pub commits: u32,
    pub prs: u32,
    pub issues: u32,
    pub comments: u32,
}

/// Calculate activity score for a repository
pub fn calculate_activity_score(metrics: &ActivityMetrics, weights: &ActivityWeights) -> u32 {
    let commit_score = metrics.commits * weights.commits;
    let pr_score = metrics.prs * weights.prs;
    let issue_score = metrics.issues * weights.issues;
    let comment_score = metrics.comments * weights.comments;

    commit_score + pr_score + issue_score + comment_score
}

/// Categorize activity level based on score
pub fn categorize_activity_level(score: u32) -> ActivityLevel {
    match score {
        0..=10 => ActivityLevel::Low,
        11..=30 => ActivityLevel::Medium,
        31..=60 => ActivityLevel::High,
        _ => ActivityLevel::VeryHigh,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityLevel {
    Low,
    Medium,
    High,
    VeryHigh,
}

impl ActivityLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            ActivityLevel::Low => "Low",
            ActivityLevel::Medium => "Medium",
            ActivityLevel::High => "High",
            ActivityLevel::VeryHigh => "Very High",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            ActivityLevel::Low => "🟢",
            ActivityLevel::Medium => "🟡",
            ActivityLevel::High => "🟠",
            ActivityLevel::VeryHigh => "🔴",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_activity_scoring() {
        let metrics = ActivityMetrics {
            commits: 5,
            prs: 3,
            issues: 2,
            comments: 10,
        };

        let weights = ActivityWeights {
            commits: 4,
            prs: 3,
            issues: 2,
            comments: 1,
        };

        let score = calculate_activity_score(&metrics, &weights);
        // 5*4 + 3*3 + 2*2 + 10*1 = 20 + 9 + 4 + 10 = 43
        assert_eq!(score, 43);

        let level = categorize_activity_level(score);
        assert_eq!(level, ActivityLevel::High);
    }

    #[test]
    fn test_activity_level_categorization() {
        assert_eq!(categorize_activity_level(5), ActivityLevel::Low);
        assert_eq!(categorize_activity_level(15), ActivityLevel::Medium);
        assert_eq!(categorize_activity_level(45), ActivityLevel::High);
        assert_eq!(categorize_activity_level(75), ActivityLevel::VeryHigh);
    }

    #[test]
    fn test_activity_level_display() {
        assert_eq!(ActivityLevel::Low.as_str(), "Low");
        assert_eq!(ActivityLevel::High.emoji(), "🟠");
    }
}
