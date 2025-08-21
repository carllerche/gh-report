use anyhow::{Context, Result};
use jiff::{Timestamp, ToSpan};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Deserialize, Serialize)]
pub struct State {
    pub last_run: Option<Timestamp>,
    pub last_report_file: Option<String>,
    /// DEPRECATED: Repository tracking is no longer used with activity-based discovery
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    #[deprecated(note = "Repository tracking is no longer used with activity-based discovery")]
    pub tracked_repos: HashMap<String, RepoState>,
}

/// DEPRECATED: RepoState is no longer used with activity-based discovery
#[derive(Debug, Deserialize, Serialize)]
#[deprecated(note = "RepoState is no longer used with activity-based discovery")]
pub struct RepoState {
    pub last_seen: Timestamp,
    pub activity_score: u32,
    pub auto_tracked: bool,
}

impl State {
    /// Load state from file
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read state from {:?}", path))?;

        let state: State = serde_json::from_str(&contents)
            .with_context(|| format!("Failed to parse state from {:?}", path))?;

        Ok(state)
    }

    /// Save state to file
    pub fn save(&self, path: &Path) -> Result<()> {
        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory {:?}", parent))?;
        }

        let contents = serde_json::to_string_pretty(self).context("Failed to serialize state")?;

        std::fs::write(path, contents)
            .with_context(|| format!("Failed to write state to {:?}", path))?;

        Ok(())
    }

    /// Update the last run timestamp to now
    pub fn update_last_run(&mut self) {
        self.last_run = Some(Timestamp::now());
    }

    /// Get the timestamp to fetch data since
    pub fn get_since_timestamp(&self, max_lookback_days: u32) -> Timestamp {
        match self.last_run {
            Some(last) => {
                let now = Timestamp::now();
                // Convert days to hours for timestamp arithmetic
                let hours = (max_lookback_days as i64) * 24;
                let max_lookback = now.saturating_sub(hours.hours()).expect("valid timestamp");

                // Use the more recent of last_run or max_lookback
                if last > max_lookback {
                    last
                } else {
                    max_lookback
                }
            }
            None => {
                // First run - look back max_lookback_days
                let hours = (max_lookback_days as i64) * 24;
                Timestamp::now()
                    .saturating_sub(hours.hours())
                    .expect("valid timestamp")
            }
        }
    }

    /// Add a repository to track
    pub fn add_repository(&mut self, repo_name: &str) {
        self.tracked_repos.insert(
            repo_name.to_string(),
            RepoState {
                last_seen: Timestamp::now(),
                activity_score: 0,
                auto_tracked: false,
            },
        );
    }

    /// Check if a repository should be auto-removed due to inactivity
    pub fn should_remove_repo(&self, repo_name: &str, threshold_days: u32) -> bool {
        if let Some(repo_state) = self.tracked_repos.get(repo_name) {
            if !repo_state.auto_tracked {
                return false; // Never auto-remove manually configured repos
            }

            let hours = (threshold_days as i64) * 24;
            let threshold = Timestamp::now()
                .saturating_sub(hours.hours())
                .expect("valid timestamp");
            repo_state.last_seen < threshold
        } else {
            false
        }
    }

    /// Update or add a repository's state
    pub fn update_repo(&mut self, name: String, score: u32, auto_tracked: bool) {
        let repo_state = RepoState {
            last_seen: Timestamp::now(),
            activity_score: score,
            auto_tracked,
        };
        self.tracked_repos.insert(name, repo_state);
    }

    /// Remove inactive repositories
    pub fn cleanup_inactive_repos(&mut self, threshold_days: u32) -> Vec<String> {
        let mut removed = Vec::new();
        let hours = (threshold_days as i64) * 24;
        let threshold = Timestamp::now()
            .saturating_sub(hours.hours())
            .expect("valid timestamp");

        self.tracked_repos.retain(|name, state| {
            if state.auto_tracked && state.last_seen < threshold {
                removed.push(name.clone());
                false
            } else {
                true
            }
        });

        removed
    }
}

impl Default for State {
    fn default() -> Self {
        State {
            last_run: None,
            last_report_file: None,
            tracked_repos: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_state_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("state.json");

        let mut state = State::default();
        state.update_last_run();
        state.update_repo("test/repo".to_string(), 42, true);

        // Save state
        state.save(&state_path).unwrap();

        // Load state
        let loaded = State::load(&state_path).unwrap();
        assert!(loaded.last_run.is_some());
        assert_eq!(loaded.tracked_repos.len(), 1);
        assert_eq!(loaded.tracked_repos["test/repo"].activity_score, 42);
    }

    #[test]
    fn test_load_nonexistent_returns_default() {
        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("nonexistent.json");

        let state = State::load(&state_path).unwrap();
        assert!(state.last_run.is_none());
        assert!(state.tracked_repos.is_empty());
    }

    #[test]
    fn test_cleanup_inactive_repos() {
        let mut state = State::default();

        // Add repos with different last_seen times
        let now = Timestamp::now();
        state.tracked_repos.insert(
            "old/repo".to_string(),
            RepoState {
                last_seen: now
                    .saturating_sub((40 * 24).hours())
                    .expect("valid timestamp"),
                activity_score: 10,
                auto_tracked: true,
            },
        );
        state.tracked_repos.insert(
            "recent/repo".to_string(),
            RepoState {
                last_seen: now
                    .saturating_sub((5 * 24).hours())
                    .expect("valid timestamp"),
                activity_score: 20,
                auto_tracked: true,
            },
        );
        state.tracked_repos.insert(
            "manual/repo".to_string(),
            RepoState {
                last_seen: now
                    .saturating_sub((60 * 24).hours())
                    .expect("valid timestamp"),
                activity_score: 5,
                auto_tracked: false, // Manual repos are never removed
            },
        );

        let removed = state.cleanup_inactive_repos(30);

        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0], "old/repo");
        assert_eq!(state.tracked_repos.len(), 2);
        assert!(state.tracked_repos.contains_key("recent/repo"));
        assert!(state.tracked_repos.contains_key("manual/repo"));
    }
}
