// Time utility functions

use crate::api_traits::Timestamp;
use crate::remote::{ListBodyArgs, ListSortMode};
use crate::Error;

use crate::error::{self, GRError};
use crate::Result;
use chrono::{DateTime, Local};
use std;
use std::fmt::{Display, Formatter};
use std::ops::{Add, AddAssign, Deref, Div, Sub};
use std::str::FromStr;
use std::time::Duration;

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

pub fn epoch_to_minutes_relative(epoch_seconds: Seconds) -> String {
    let now = now_epoch_seconds();
    let diff = now - epoch_seconds;
    let minutes = diff / Seconds::new(60);
    minutes.to_string()
}

pub fn epoch_to_seconds_relative(epoch_seconds: Seconds) -> String {
    let now = now_epoch_seconds();
    let diff = now - epoch_seconds;
    diff.to_string()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct Seconds(u64);

impl Seconds {
    pub fn new(seconds: u64) -> Self {
        Seconds(seconds)
    }
}

impl FromStr for Seconds {
    type Err = GRError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.parse::<u64>() {
            Ok(seconds) => Ok(Seconds(seconds)),
            Err(err) => Err(GRError::TimeConversionError(format!(
                "Could not convert {} to time format: {}",
                s, err,
            ))),
        }
    }
}

impl Sub<Seconds> for Seconds {
    type Output = Seconds;

    fn sub(self, rhs: Seconds) -> Self::Output {
        if self.0 < rhs.0 {
            return Seconds(rhs.0 - self.0);
        }
        Seconds(self.0 - rhs.0)
    }
}

impl Add<Seconds> for Seconds {
    type Output = Seconds;

    fn add(self, rhs: Seconds) -> Self::Output {
        Seconds(self.0 + rhs.0)
    }
}

impl Div<Seconds> for Seconds {
    type Output = Seconds;

    fn div(self, rhs: Seconds) -> Self::Output {
        Seconds(self.0 / rhs.0)
    }
}

impl Deref for Seconds {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<u64> for Seconds {
    fn from(seconds: u64) -> Self {
        Seconds(seconds)
    }
}

impl Display for Seconds {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, PartialOrd, Ord)]
pub struct Milliseconds(u64);

impl Milliseconds {
    pub fn new(milliseconds: u64) -> Self {
        Milliseconds(milliseconds)
    }
}

impl Deref for Milliseconds {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<u64> for Milliseconds {
    fn from(milliseconds: u64) -> Self {
        Milliseconds(milliseconds)
    }
}

impl From<Milliseconds> for Duration {
    fn from(milliseconds: Milliseconds) -> Self {
        Duration::from_millis(milliseconds.0)
    }
}

impl From<Seconds> for Milliseconds {
    fn from(seconds: Seconds) -> Self {
        Milliseconds(seconds.0 * 1000)
    }
}

impl Add<Milliseconds> for Milliseconds {
    type Output = Milliseconds;

    fn add(self, rhs: Milliseconds) -> Self::Output {
        Milliseconds(self.0 + rhs.0)
    }
}

impl AddAssign<Milliseconds> for Milliseconds {
    fn add_assign(&mut self, rhs: Milliseconds) {
        self.0 += rhs.0;
    }
}

impl Display for Milliseconds {
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
        let (created_after, created_before) = (list_args.created_after, list_args.created_before);
        match (created_after, created_before) {
            (Some(created_after), Some(created_before)) => {
                let created_after = created_after.parse::<DateTime<Local>>().map_err(|err| {
                    GRError::TimeConversionError(format!(
                        "Could not convert {} to date format: {}",
                        created_after, err,
                    ))
                })?;
                let created_before = created_before.parse::<DateTime<Local>>().map_err(|err| {
                    GRError::TimeConversionError(format!(
                        "Could not convert {} to date format: {}",
                        created_before, err,
                    ))
                })?;
                return Ok(sort_by_date(
                    data,
                    Some(created_after),
                    Some(created_before),
                    Some(list_args.sort_mode),
                ));
            }
            (Some(created_after), None) => {
                let created_after = created_after.parse::<DateTime<Local>>().map_err(|err| {
                    GRError::TimeConversionError(format!(
                        "Could not convert {} to date format: {}",
                        created_after, err,
                    ))
                })?;
                return Ok(sort_by_date(
                    data,
                    Some(created_after),
                    None,
                    Some(list_args.sort_mode),
                ));
            }
            (None, Some(created_before)) => {
                let created_before = created_before.parse::<DateTime<Local>>().map_err(|err| {
                    GRError::TimeConversionError(format!(
                        "Could not convert {} to date format: {}",
                        created_before, err,
                    ))
                })?;
                return Ok(sort_by_date(
                    data,
                    None,
                    Some(created_before),
                    Some(list_args.sort_mode),
                ));
            }
            (None, None) => {
                return Ok(sort_by_date(data, None, None, Some(list_args.sort_mode)));
            }
        }
    }
    Ok(sort_by_date(data, None, None, Some(ListSortMode::Asc)))
}

fn sort_by_date<T: Timestamp>(
    data: Vec<T>,
    created_after: Option<DateTime<Local>>,
    created_before: Option<DateTime<Local>>,
    sort_mode: Option<ListSortMode>,
) -> Vec<T> {
    let mut data_dates = match (created_after, created_before) {
        (Some(created_after), Some(created_before)) => data
            .into_iter()
            .filter_map(|item| {
                let item_date = item.created_at().parse::<DateTime<Local>>().ok()?;
                if item_date >= created_after && item_date <= created_before {
                    return Some((item, item_date));
                }
                None
            })
            .collect::<Vec<(T, DateTime<Local>)>>(),
        (Some(created_after), None) => data
            .into_iter()
            .filter_map(|item| {
                let item_date = item.created_at().parse::<DateTime<Local>>().ok()?;
                if item_date >= created_after {
                    return Some((item, item_date));
                }
                None
            })
            .collect::<Vec<(T, DateTime<Local>)>>(),
        (None, Some(created_before)) => data
            .into_iter()
            .filter_map(|item| {
                let item_date = item.created_at().parse::<DateTime<Local>>().ok()?;
                if item_date <= created_before {
                    return Some((item, item_date));
                }
                None
            })
            .collect::<Vec<(T, DateTime<Local>)>>(),
        (None, None) => data
            .into_iter()
            .map(|item| {
                let item_date = item.created_at().parse::<DateTime<Local>>().unwrap();
                (item, item_date)
            })
            .collect::<Vec<(T, DateTime<Local>)>>(),
    };
    if let Some(sort_mode) = sort_mode {
        match sort_mode {
            ListSortMode::Asc => data_dates.sort_by(|a, b| a.1.cmp(&b.1)),
            ListSortMode::Desc => data_dates.sort_by(|a, b| b.1.cmp(&a.1)),
        }
    }
    data_dates.into_iter().map(|(item, _)| item).collect()
}

pub fn compute_duration(start: &str, end: &str) -> u64 {
    let created_at = chrono::DateTime::parse_from_rfc3339(start).unwrap();
    let updated_at = chrono::DateTime::parse_from_rfc3339(end).unwrap();
    updated_at.signed_duration_since(created_at).num_seconds() as u64
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
        assert_eq!(3, filtered.len());
        assert_eq!("2021-01-01T00:00:00Z", filtered[0].created_at());
        assert_eq!("2021-02-02T00:00:00Z", filtered[1].created_at());
        assert_eq!("2021-03-02T00:00:00Z", filtered[2].created_at());
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
        assert_eq!(3, sorted.len());
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

    #[test]
    fn test_sort_by_date_descending_order() {
        let data = vec![
            TimestampMock::new("2021-01-01T00:00:00Z"),
            TimestampMock::new("2020-12-31T00:00:00Z"),
            TimestampMock::new("2021-01-02T00:00:00Z"),
        ];
        let sorted = sort_by_date(data, None, None, Some(ListSortMode::Desc));
        assert_eq!(3, sorted.len());
        assert_eq!("2021-01-02T00:00:00Z", sorted[0].created_at());
        assert_eq!("2021-01-01T00:00:00Z", sorted[1].created_at());
        assert_eq!("2020-12-31T00:00:00Z", sorted[2].created_at());
    }

    #[test]
    fn test_filter_by_created_before_date() {
        let created_before = "2021-01-01T00:00:00Z".to_string();
        let list_args = ListBodyArgs::builder()
            .created_before(Some(created_before))
            .build()
            .unwrap();
        let data = vec![
            TimestampMock::new("2021-01-01T00:00:00Z"),
            TimestampMock::new("2020-12-31T00:00:00Z"),
            TimestampMock::new("2021-03-02T00:00:00Z"),
            TimestampMock::new("2021-02-02T00:00:00Z"),
        ];
        let filtered = sort_filter_by_date(data, Some(list_args)).unwrap();
        assert_eq!(2, filtered.len());
        assert_eq!("2020-12-31T00:00:00Z", filtered[0].created_at());
        assert_eq!("2021-01-01T00:00:00Z", filtered[1].created_at());
    }

    #[test]
    fn test_filter_by_created_after_and_created_before_date() {
        let created_after = "2021-01-01T00:00:00Z".to_string();
        let created_before = "2021-02-01T00:00:00Z".to_string();
        let list_args = ListBodyArgs::builder()
            .created_after(Some(created_after))
            .created_before(Some(created_before))
            .build()
            .unwrap();
        let data = vec![
            TimestampMock::new("2021-01-01T00:00:00Z"),
            TimestampMock::new("2021-01-20T00:00:00Z"),
            TimestampMock::new("2020-12-31T00:00:00Z"),
            TimestampMock::new("2021-03-02T00:00:00Z"),
            TimestampMock::new("2021-02-02T00:00:00Z"),
        ];
        let filtered = sort_filter_by_date(data, Some(list_args)).unwrap();
        assert_eq!(2, filtered.len());
        assert_eq!("2021-01-01T00:00:00Z", filtered[0].created_at());
        assert_eq!("2021-01-20T00:00:00Z", filtered[1].created_at());
    }

    #[test]
    fn test_no_filter_with_no_created_after_and_no_created_before() {
        let data = vec![
            TimestampMock::new("2021-01-01T00:00:00Z"),
            TimestampMock::new("2020-12-31T00:00:00Z"),
            TimestampMock::new("2021-03-02T00:00:00Z"),
            TimestampMock::new("2021-02-02T00:00:00Z"),
        ];
        let filtered = sort_filter_by_date(data, None).unwrap();
        assert_eq!(4, filtered.len());
        assert_eq!("2020-12-31T00:00:00Z", filtered[0].created_at());
        assert_eq!("2021-01-01T00:00:00Z", filtered[1].created_at());
        assert_eq!("2021-02-02T00:00:00Z", filtered[2].created_at());
        assert_eq!("2021-03-02T00:00:00Z", filtered[3].created_at());
    }

    #[test]
    fn test_error_if_created_before_invalid_non_iso_8601_date() {
        let created_before = "2021-01/01".to_string();
        let list_args = ListBodyArgs::builder()
            .created_before(Some(created_before))
            .build()
            .unwrap();
        let data = vec![
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

    #[test]
    fn test_created_after_and_before_available_after_is_invalid_date() {
        let created_after = "2021-01/01".to_string();
        let created_before = "2021-01-01T00:00:00Z".to_string();
        let list_args = ListBodyArgs::builder()
            .created_after(Some(created_after))
            .created_before(Some(created_before))
            .build()
            .unwrap();
        let data = vec![
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

    #[test]
    fn test_created_after_and_before_available_before_is_invalid_date() {
        let created_after = "2021-01-01T00:00:00Z".to_string();
        let created_before = "2021-01/01".to_string();
        let list_args = ListBodyArgs::builder()
            .created_after(Some(created_after))
            .created_before(Some(created_before))
            .build()
            .unwrap();
        let data = vec![
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

    #[test]
    fn test_compute_duration() {
        let created_at = "2020-01-01T00:00:00Z";
        let updated_at = "2020-01-01T00:01:00Z";
        let duration = compute_duration(created_at, updated_at);
        assert_eq!(60, duration);
    }
}
