use anyhow::{anyhow, Result};
use std::str::FromStr;

/// Parse time duration from a string with optional suffix
///
/// Examples:
/// - "3h" or "3H" → 3 hours → 3 * 24 = 72 hours in days
/// - "3d" or "3D" → 3 days
/// - "3w" or "3W" → 3 weeks → 3 * 7 = 21 days
/// - "3" → 3 days (default)
#[derive(Debug, Clone, PartialEq)]
pub struct TimeDuration {
    pub days: u32,
}

impl TimeDuration {
    /// Convert to days as u32 (used by existing API)
    pub fn as_days(&self) -> u32 {
        self.days
    }

    /// Convert hours to days (rounded up)
    fn hours_to_days(hours: u32) -> u32 {
        (hours + 23) / 24 // Round up: 1-24h = 1 day, 25-48h = 2 days, etc.
    }

    /// Convert weeks to days
    fn weeks_to_days(weeks: u32) -> u32 {
        weeks * 7
    }
}

impl FromStr for TimeDuration {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let s = s.trim();

        if s.is_empty() {
            return Err(anyhow!("Time duration cannot be empty"));
        }

        // Check for suffix
        let (number_part, suffix) = if let Some(last_char) = s.chars().last() {
            if last_char.is_ascii_alphabetic() {
                let (num, suf) = s.split_at(s.len() - 1);
                (num, Some(suf.to_lowercase()))
            } else {
                (s, None)
            }
        } else {
            (s, None)
        };

        // Parse the numeric part
        let number: u32 = number_part
            .parse()
            .map_err(|_| anyhow!("Invalid number in time duration: '{}'", number_part))?;

        if number == 0 {
            return Err(anyhow!("Time duration must be greater than 0"));
        }

        // Convert to days based on suffix
        let days = match suffix.as_deref() {
            Some("h") => Self::hours_to_days(number),
            Some("d") | None => number, // Default to days
            Some("w") => Self::weeks_to_days(number),
            Some(other) => {
                return Err(anyhow!(
                    "Invalid time suffix '{}'. Use 'h' for hours, 'd' for days, or 'w' for weeks",
                    other
                ))
            }
        };

        Ok(TimeDuration { days })
    }
}

impl std::fmt::Display for TimeDuration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} days", self.days)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_days() {
        assert_eq!("3d".parse::<TimeDuration>().unwrap().as_days(), 3);
        assert_eq!("3D".parse::<TimeDuration>().unwrap().as_days(), 3);
        assert_eq!("3".parse::<TimeDuration>().unwrap().as_days(), 3);
        assert_eq!("1".parse::<TimeDuration>().unwrap().as_days(), 1);
    }

    #[test]
    fn test_parse_hours() {
        assert_eq!("1h".parse::<TimeDuration>().unwrap().as_days(), 1);
        assert_eq!("12h".parse::<TimeDuration>().unwrap().as_days(), 1);
        assert_eq!("24h".parse::<TimeDuration>().unwrap().as_days(), 1);
        assert_eq!("25h".parse::<TimeDuration>().unwrap().as_days(), 2);
        assert_eq!("48h".parse::<TimeDuration>().unwrap().as_days(), 2);
    }

    #[test]
    fn test_parse_weeks() {
        assert_eq!("1w".parse::<TimeDuration>().unwrap().as_days(), 7);
        assert_eq!("2w".parse::<TimeDuration>().unwrap().as_days(), 14);
        assert_eq!("3W".parse::<TimeDuration>().unwrap().as_days(), 21);
    }

    #[test]
    fn test_parse_errors() {
        assert!("".parse::<TimeDuration>().is_err());
        assert!("0".parse::<TimeDuration>().is_err());
        assert!("0d".parse::<TimeDuration>().is_err());
        assert!("abc".parse::<TimeDuration>().is_err());
        assert!("3x".parse::<TimeDuration>().is_err());
        assert!("3.5d".parse::<TimeDuration>().is_err());
    }
}
