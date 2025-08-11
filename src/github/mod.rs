use anyhow::{anyhow, Context, Result};
use std::process::Command;

mod models;
mod client;

pub use models::*;
pub use client::*;

#[cfg(test)]
pub use client::MockGitHub;

/// Minimum supported gh CLI version
pub const MIN_GH_VERSION: &str = "2.20.0";

/// Check if gh CLI is installed and meets minimum version requirement
pub fn check_gh_version() -> Result<String> {
    let output = Command::new("gh")
        .arg("version")
        .output()
        .context("Failed to run 'gh version'. Is GitHub CLI installed?")?;

    if !output.status.success() {
        return Err(anyhow!("gh version command failed"));
    }

    let version_output = String::from_utf8_lossy(&output.stdout);
    let version = parse_gh_version(&version_output)?;
    
    if !version_meets_minimum(&version, MIN_GH_VERSION)? {
        return Err(anyhow!(
            "gh version {} is too old. Minimum required: {}",
            version,
            MIN_GH_VERSION
        ));
    }

    Ok(version)
}

/// Parse version from gh version output
fn parse_gh_version(output: &str) -> Result<String> {
    // gh version output format: "gh version 2.32.0 (2023-06-20)"
    let version = output
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(2))
        .ok_or_else(|| anyhow!("Could not parse gh version from output"))?;
    
    Ok(version.to_string())
}

/// Check if version meets minimum requirement
fn version_meets_minimum(version: &str, minimum: &str) -> Result<bool> {
    let version_parts = parse_version_parts(version)?;
    let minimum_parts = parse_version_parts(minimum)?;
    
    // Compare major.minor.patch
    for i in 0..3 {
        let v = version_parts.get(i).unwrap_or(&0);
        let m = minimum_parts.get(i).unwrap_or(&0);
        
        if v > m {
            return Ok(true);
        } else if v < m {
            return Ok(false);
        }
        // If equal, continue to next part
    }
    
    Ok(true) // Versions are equal
}

/// Parse version string into numeric parts
fn parse_version_parts(version: &str) -> Result<Vec<u32>> {
    version
        .split('.')
        .map(|part| {
            part.parse::<u32>()
                .with_context(|| format!("Invalid version part: {}", part))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_gh_version() {
        let output = "gh version 2.32.0 (2023-06-20)\n";
        assert_eq!(parse_gh_version(output).unwrap(), "2.32.0");
    }

    #[test]
    fn test_version_comparison() {
        assert!(version_meets_minimum("2.32.0", "2.20.0").unwrap());
        assert!(version_meets_minimum("3.0.0", "2.20.0").unwrap());
        assert!(version_meets_minimum("2.20.0", "2.20.0").unwrap());
        assert!(!version_meets_minimum("2.19.0", "2.20.0").unwrap());
        assert!(!version_meets_minimum("1.99.99", "2.0.0").unwrap());
    }
}