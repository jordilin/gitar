// Time utility functions

use crate::api_traits::Timestamp;
use crate::remote::ListBodyArgs;
use crate::Error;

use crate::error::{self, GRError};
use crate::Result;
use chrono::{DateTime, Local};
use std;
use std::fmt::{Display, Formatter};
use std::ops::{Add, Deref, Sub};

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
    type Error = Error;

    fn try_from(time: char) -> std::result::Result<Self, Self::Error> {
        match time {
            's' => Ok(Time::Second),
            'm' => Ok(Time::Minute),
            'h' => Ok(Time::Hour),
            'd' => Ok(Time::Day),
            _ => Err(error::gen(format!(
                "Unknown char time format: {} - valid types are s, m, h, d",
                time
            ))),
        }
    }
}

pub fn now_epoch_seconds() -> Seconds {
    let now_epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    Seconds(now_epoch)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
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

impl Add<Seconds> for Seconds {
    type Output = Seconds;

    fn add(self, rhs: Seconds) -> Self::Output {
        Seconds(self.0 + rhs.0)
    }
}

impl Deref for Seconds {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for Seconds {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Convert a string with time format to seconds.
/// A string with time format can be anything like:
/// 1s, 2s, 2 seconds, 2 second, 2seconds, 2second, 2 s
/// The same would apply for minutes, hours and days
/// Processing stops at the first non-digit character
fn string_to_seconds(str_fmt: &str) -> Result<Seconds> {
    let mut seconds: u64 = 0;
    for c in str_fmt.chars() {
        if c.is_ascii_digit() {
            seconds = seconds * 10 + c.to_digit(10).unwrap() as u64;
        } else {
            if c.is_whitespace() {
                continue;
            }
            seconds *= Time::try_from(c)?.to_seconds();
            break;
        }
    }
    Ok(Seconds(seconds))
}

impl TryFrom<&str> for Seconds {
    type Error = GRError;

    fn try_from(str_fmt: &str) -> std::result::Result<Self, Self::Error> {
        match string_to_seconds(str_fmt) {
            Ok(seconds) => Ok(seconds),
            Err(err) => Err(GRError::TimeConversionError(format!(
                "Could not convert {} to time format: {}",
                str_fmt, err,
            ))),
        }
    }
}

pub fn sort_filter_by_date<T: Timestamp>(
    data: Vec<T>,
    list_args: Option<ListBodyArgs>,
) -> Result<Vec<T>> {
    if let Some(list_args) = list_args {
        let date = list_args.created_after;
        if date.is_some() {
            let created_after =
                date.as_ref()
                    .unwrap()
                    .parse::<DateTime<Local>>()
                    .map_err(|err| {
                        GRError::TimeConversionError(format!(
                            "Could not convert {} to date format: {}",
                            date.unwrap(),
                            err,
                        ))
                    })?;
            return Ok(sort_by_date(data, Some(created_after), true));
        }
    }
    Ok(sort_by_date(data, None, false))
}

fn sort_by_date<T: Timestamp>(data: Vec<T>, date: Option<DateTime<Local>>, filter: bool) -> Vec<T> {
    let mut data_dates = if filter {
        data.into_iter()
            .filter_map(|item| {
                let item_date = item.created_at().parse::<DateTime<Local>>().ok()?;
                if item_date > date.unwrap() {
                    return Some((item, item_date));
                }
                None
            })
            .collect::<Vec<(T, DateTime<Local>)>>()
    } else {
        data.into_iter()
            .map(|item| {
                let item_date = item.created_at().parse::<DateTime<Local>>().unwrap();
                (item, item_date)
            })
            .collect::<Vec<(T, DateTime<Local>)>>()
    };
    data_dates.sort_by(|a, b| a.1.cmp(&b.1));
    data_dates.into_iter().map(|(item, _)| item).collect()
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

    struct TimestampMock {
        created_at: String,
    }

    impl TimestampMock {
        fn new(created_at: &str) -> Self {
            TimestampMock {
                created_at: created_at.to_string(),
            }
        }
    }

    impl Timestamp for TimestampMock {
        fn created_at(&self) -> String {
            self.created_at.clone()
        }
    }

    #[test]
    fn test_filter_date_created_after_iso_8601() {
        let created_after = "2021-01-01T00:00:00Z".to_string();
        let list_args = ListBodyArgs::builder()
            .created_after(Some(created_after))
            .build()
            .unwrap();
        let data = vec![
            TimestampMock::new("2021-01-01T00:00:00Z"),
            TimestampMock::new("2020-12-31T00:00:00Z"),
            TimestampMock::new("2021-03-02T00:00:00Z"),
            TimestampMock::new("2021-02-02T00:00:00Z"),
        ];
        let filtered = sort_filter_by_date(data, Some(list_args)).unwrap();
        assert_eq!("2021-02-02T00:00:00Z", filtered[0].created_at());
        assert_eq!("2021-03-02T00:00:00Z", filtered[1].created_at());
    }

    #[test]
    fn test_filter_date_created_after_iso_8601_no_date() {
        let data = vec![
            TimestampMock::new("2021-01-01T00:00:00Z"),
            TimestampMock::new("2020-12-31T00:00:00Z"),
            TimestampMock::new("2021-01-02T00:00:00Z"),
        ];
        // no filter, just data sort ascending.
        let sorted = sort_filter_by_date(data, None).unwrap();
        assert_eq!("2020-12-31T00:00:00Z", sorted[0].created_at());
        assert_eq!("2021-01-01T00:00:00Z", sorted[1].created_at());
        assert_eq!("2021-01-02T00:00:00Z", sorted[2].created_at());
    }

    #[test]
    fn test_filter_date_created_at_iso_8601_invalid_date_filtered_out() {
        let created_after = "2021-01-01T00:00:00Z".to_string();
        let list_args = ListBodyArgs::builder()
            .created_after(Some(created_after))
            .build()
            .unwrap();
        let data = vec![
            TimestampMock::new("2021-01/01"),
            TimestampMock::new("2020-12-31T00:00:00Z"),
            TimestampMock::new("2021-01-02T00:00:00Z"),
        ];
        let filtered = sort_filter_by_date(data, Some(list_args)).unwrap();
        assert_eq!(1, filtered.len());
    }

    #[test]
    fn test_created_after_invalid_date_is_error() {
        let created_after = "2021-01/01".to_string();
        let list_args = ListBodyArgs::builder()
            .created_after(Some(created_after))
            .build()
            .unwrap();
        let data = vec![
            TimestampMock::new("2021-01/01"),
            TimestampMock::new("2020-12-31T00:00:00Z"),
            TimestampMock::new("2021-01-02T00:00:00Z"),
        ];
        let result = sort_filter_by_date(data, Some(list_args));
        match result {
            Err(err) => match err.downcast_ref::<GRError>() {
                Some(GRError::TimeConversionError(_)) => (),
                _ => panic!("Expected TimeConversionError"),
            },
            _ => panic!("Expected TimeConversionError"),
        }
    }
}
