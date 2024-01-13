// Time utility functions

use crate::error::GRError;
use crate::Result;
use std;
use std::ops::{Deref, Sub};

enum Time {
    Second,
    Minute,
    Hour,
    Day,
}

impl Time {
    fn to_seconds(&self) -> u64 {
        match self {
            Time::Second => 1,
            Time::Minute => 60,
            Time::Hour => 3600,
            Time::Day => 86400,
        }
    }
}

impl TryFrom<char> for Time {
    type Error = GRError;

    fn try_from(time: char) -> std::result::Result<Self, Self::Error> {
        match time {
            's' => Ok(Time::Second),
            'm' => Ok(Time::Minute),
            'h' => Ok(Time::Hour),
            'd' => Ok(Time::Day),
            _ => Err(GRError::ConfigurationError(format!(
                "Unknown char time format: {} - valid types are s, m, h, d",
                time
            ))),
        }
    }
}

#[derive(Debug, PartialEq, PartialOrd)]
pub struct Seconds(u64);

impl Seconds {
    pub fn new(seconds: u64) -> Self {
        Seconds(seconds)
    }
}

impl Sub<Seconds> for Seconds {
    type Output = Seconds;

    fn sub(self, rhs: Seconds) -> Self::Output {
        Seconds(self.0 - rhs.0)
    }
}

impl Deref for Seconds {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Convert a string with time format to seconds.
/// A string with time format can be anything like:
/// 1s, 2s, 2 seconds, 2 second, 2seconds, 2second, 2 s
/// The same would apply for minutes, hours and days
/// Processing stops at the first non-digit character
pub fn string_to_seconds(str_fmt: &str) -> Result<Seconds> {
    let mut seconds: u64 = 0;
    for c in str_fmt.chars() {
        if c.is_digit(10) {
            seconds = seconds * 10 + c.to_digit(10).unwrap() as u64;
        } else {
            if c.is_whitespace() {
                continue;
            }
            seconds = seconds * Time::try_from(c)?.to_seconds();
            break;
        }
    }
    Ok(Seconds(seconds))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_formatted_string_to_seconds() {
        let test_table = vec![
            ("1s", Seconds(1)),
            ("2s", Seconds(2)),
            ("2 seconds", Seconds(2)),
            ("2 second", Seconds(2)),
            ("2seconds", Seconds(2)),
            ("2second", Seconds(2)),
            ("2 s", Seconds(2)),
            ("1m", Seconds(60)),
            ("2m", Seconds(120)),
            ("2 minutes", Seconds(120)),
            ("2 minute", Seconds(120)),
            ("2minutes", Seconds(120)),
            ("2minute", Seconds(120)),
            ("2 m", Seconds(120)),
            ("1h", Seconds(3600)),
            ("2h", Seconds(7200)),
            ("2 hours", Seconds(7200)),
            ("2 hour", Seconds(7200)),
            ("2hours", Seconds(7200)),
            ("2hour", Seconds(7200)),
            ("2 h", Seconds(7200)),
            ("1d", Seconds(86400)),
            ("2d", Seconds(172800)),
            ("2 days", Seconds(172800)),
            ("2 day", Seconds(172800)),
            ("2days", Seconds(172800)),
            ("2day", Seconds(172800)),
            ("2 d", Seconds(172800)),
            // If no time format is specified, it defaults to seconds
            ("300", Seconds(300)),
            // empty string is zero
            ("", Seconds(0)),
        ];
        for (input, expected) in test_table {
            let actual = string_to_seconds(input).unwrap();
            assert_eq!(expected.0, actual.0);
        }
    }

    #[test]
    fn test_cannot_convert_time_formatted_string_to_seconds() {
        let input_err = "2x"; // user meant 2d and typed 2x
        assert!(string_to_seconds(input_err).is_err());
    }
}
