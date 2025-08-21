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

    }

}
