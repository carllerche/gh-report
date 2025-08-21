use anyhow::{Context, Result};
use jiff::{Timestamp, ToSpan};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Deserialize, Serialize)]
pub struct State {
    pub last_run: Option<Timestamp>,
    pub last_report_file: Option<String>,
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
}

impl Default for State {
    fn default() -> Self {
        State {
            last_run: None,
            last_report_file: None,
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

        // Save state
        state.save(&state_path).unwrap();

        // Load state
        let loaded = State::load(&state_path).unwrap();
        assert!(loaded.last_run.is_some());
    }

    #[test]
    fn test_load_nonexistent_returns_default() {
        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("nonexistent.json");

        let state = State::load(&state_path).unwrap();
        assert!(state.last_run.is_none());
    }
}
