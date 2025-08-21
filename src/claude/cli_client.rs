use anyhow::{anyhow, Context, Result};
use std::io::Write;
use std::process::Command;
use tracing::{debug, info};

/// Claude CLI client - uses the `claude` command-line tool
pub struct ClaudeCLI {
    model: String,
}

impl ClaudeCLI {
    /// Create a new Claude CLI client
    pub fn new(model: String) -> Result<Self> {
        // Get claude path
        let claude_path = Self::get_claude_path()?;

        // Check claude version
        let version_output = Command::new(&claude_path)
            .arg("--version")
            .output()
            .context("Failed to get claude version")?;

        if version_output.status.success() {
            let version = String::from_utf8_lossy(&version_output.stdout);
            info!("Using claude CLI version: {}", version.trim());
        }

        Ok(ClaudeCLI { model })
    }

    /// Send a message to Claude via CLI
    pub fn send_message(&self, prompt: &str, system_prompt: Option<&str>) -> Result<String> {
        debug!("Sending message to Claude CLI with {} chars", prompt.len());

        // Get the claude binary path
        let claude_path = Self::get_claude_path()?;

        // Build the command
        let mut cmd = Command::new(&claude_path);

        // Add model flag if specified
        if !self.model.is_empty() && self.model != "default" {
            // Map our model names to claude CLI model names
            let cli_model = match self.model.as_str() {
                "claude-3-5-sonnet-20241022" | "sonnet" => "claude-3-5-sonnet",
                "claude-3-5-haiku-20241022" | "haiku" => "claude-3-5-haiku",
                "claude-3-opus-20240229" | "opus" => "claude-3-opus",
                other => other,
            };
            cmd.arg("--model").arg(cli_model);
        }

        // Add system prompt if provided
        if let Some(sys) = system_prompt {
            // For claude CLI, we'll prepend the system context to the user message
            // since the CLI doesn't have a direct system prompt flag
            let combined = format!("System context: {}\n\nUser request: {}", sys, prompt);

            // Use stdin to send the prompt to avoid shell escaping issues
            cmd.stdin(std::process::Stdio::piped());
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::piped());

            let mut child = cmd.spawn().context("Failed to spawn claude CLI")?;

            // Write prompt to stdin
            if let Some(mut stdin) = child.stdin.take() {
                stdin
                    .write_all(combined.as_bytes())
                    .context("Failed to write prompt to claude CLI")?;
            }

            let output = child
                .wait_with_output()
                .context("Failed to wait for claude CLI")?;

            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

            debug!("Claude CLI stdout: {} chars", stdout.len());
            debug!(
                "Claude CLI stderr: {}",
                if stderr.is_empty() {
                    "(empty)"
                } else {
                    &stderr
                }
            );

            if !output.status.success() {
                return Err(anyhow!(
                    "claude CLI failed with exit code {:?}: stderr={}",
                    output.status.code(),
                    stderr
                ));
            }

            if stdout.is_empty() {
                return Err(anyhow!(
                    "claude CLI returned empty output. stderr={}",
                    if stderr.is_empty() {
                        "(empty)"
                    } else {
                        &stderr
                    }
                ));
            }

            Ok(stdout)
        } else {
            // Just send the prompt directly
            cmd.stdin(std::process::Stdio::piped());
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::piped());

            let mut child = cmd.spawn().context("Failed to spawn claude CLI")?;

            if let Some(mut stdin) = child.stdin.take() {
                stdin
                    .write_all(prompt.as_bytes())
                    .context("Failed to write prompt to claude CLI")?;
            }

            let output = child
                .wait_with_output()
                .context("Failed to wait for claude CLI")?;

            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

            debug!("Claude CLI stdout: {} chars", stdout.len());
            debug!(
                "Claude CLI stderr: {}",
                if stderr.is_empty() {
                    "(empty)"
                } else {
                    &stderr
                }
            );

            if !output.status.success() {
                return Err(anyhow!(
                    "claude CLI failed with exit code {:?}: stderr={}",
                    output.status.code(),
                    stderr
                ));
            }

            if stdout.is_empty() {
                return Err(anyhow!(
                    "claude CLI returned empty output. stderr={}",
                    if stderr.is_empty() {
                        "(empty)"
                    } else {
                        &stderr
                    }
                ));
            }

            Ok(stdout)
        }
    }

    /// Check if Claude CLI is available
    pub fn is_available() -> bool {
        // Try to find claude in PATH or common locations
        if let Ok(output) = Command::new("which").arg("claude").output() {
            if output.status.success() {
                return true;
            }
        }

        // Check common installation paths
        let paths = [
            "/opt/homebrew/bin/claude",
            "/usr/local/bin/claude",
            "/usr/bin/claude",
        ];

        for path in &paths {
            if std::path::Path::new(path).exists() {
                return true;
            }
        }

        false
    }

    /// Get the path to the claude binary
    fn get_claude_path() -> Result<String> {
        // First try which
        if let Ok(output) = Command::new("which").arg("claude").output() {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    return Ok(path);
                }
            }
        }

        // Check common paths
        let paths = [
            "/opt/homebrew/bin/claude",
            "/usr/local/bin/claude",
            "/usr/bin/claude",
        ];

        for path in &paths {
            if std::path::Path::new(path).exists() {
                return Ok(path.to_string());
            }
        }

        Err(anyhow!("claude CLI not found in PATH or common locations"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_cli_availability() {
        // This test will pass or fail based on whether claude CLI is installed
        let available = ClaudeCLI::is_available();
        println!("Claude CLI available: {}", available);
    }
}
