use chrono::{DateTime, NaiveDateTime, SecondsFormat, Utc};
use diesel::{
    backend::Backend,
    deserialize::{self, FromSql, FromSqlRow},
    expression::AsExpression,
    pg::Pg,
    serialize::{self, Output, ToSql},
    sql_types::Timestamptz,
};
use error_stack::{Report, ResultExt};
use std::fmt;
use thiserror::Error;

/// A UTC timestamp wrapper around [`DateTime<Utc>`] for database compatibility.
///
/// # Formatting
/// Timestamps are serialized and formatted according to [RFC 3339] specifications
/// (for example, `YYYY-MM-DDTHH:MM:SS.SSSZ`).
///
/// [RFC 3339]: https://www.rfc-editor.org/rfc/rfc3339
#[derive(AsExpression, Clone, Copy, Debug, Eq, FromSqlRow, Hash, Ord, PartialEq, PartialOrd)]
#[diesel(sql_type = Timestamptz)]
pub struct Timestamp(DateTime<Utc>);

impl Timestamp {
    /// Creates a [`Timestamp`] representing the current system date and time in UTC.
    #[must_use]
    pub fn now() -> Self {
        Self(Utc::now())
    }

    /// Parses a [`Timestamp`] from an [RFC 3339] date and time string.
    ///
    /// # Errors
    /// Returns [`ParseError::Format`] if the input string fails to
    /// satisfy the [RFC 3339] specification.
    ///
    /// [RFC 3339]: https://www.rfc-editor.org/rfc/rfc3339
    pub fn parse(input: &str) -> Result<Self, Report<ParseError>> {
        DateTime::parse_from_rfc3339(input)
            .map(|v| v.into())
            .change_context(ParseError::Format)
    }

    /// Constructs a [`Timestamp`] from seconds since the Unix epoch (`1970-01-01T00:00:00Z`).
    ///
    /// # Errors
    /// Returns [`ParseError::Range`] if the seconds value is outside
    /// the representable range.
    pub fn from_secs(secs: i64) -> Result<Self, Report<ParseError>> {
        DateTime::from_timestamp(secs, 0)
            .map(Self)
            .ok_or_else(|| Report::new(ParseError::Range))
    }

    /// Constructs a [`Timestamp`] from milliseconds since the Unix epoch (`1970-01-01T00:00:00Z`).
    ///
    /// # Errors
    /// Returns [`ParseError::Range`] if the milliseconds value is outside
    /// the representable range.
    pub fn from_millis(millis: i64) -> Result<Self, Report<ParseError>> {
        DateTime::from_timestamp_millis(millis)
            .map(Self)
            .ok_or_else(|| Report::new(ParseError::Range))
    }

    /// Constructs a [`Timestamp`] from microseconds since the Unix epoch (`1970-01-01T00:00:00Z`).
    ///
    /// # Errors
    /// Returns [`ParseError::Range`] if the microseconds value is outside
    /// the representable range.
    pub fn from_micros(micros: i64) -> Result<Self, Report<ParseError>> {
        DateTime::from_timestamp_micros(micros)
            .map(Self)
            .ok_or_else(|| Report::new(ParseError::Range))
    }

    /// Returns the number of seconds since the Unix epoch (`1970-01-01T00:00:00Z`).
    #[must_use]
    pub const fn timestamp(&self) -> i64 {
        self.0.timestamp()
    }

    /// Returns the number of milliseconds since the Unix epoch (`1970-01-01T00:00:00Z`).
    #[must_use]
    pub const fn timestamp_millis(&self) -> i64 {
        self.0.timestamp_millis()
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let str = self.0.to_rfc3339_opts(SecondsFormat::Millis, true);
        fmt::Display::fmt(&str, f)
    }
}

impl From<Timestamp> for NaiveDateTime {
    fn from(value: Timestamp) -> Self {
        value.0.naive_utc()
    }
}

impl From<Timestamp> for DateTime<Utc> {
    fn from(value: Timestamp) -> Self {
        value.0
    }
}

impl<Tz: chrono::TimeZone> From<DateTime<Tz>> for Timestamp {
    fn from(value: DateTime<Tz>) -> Self {
        Self(value.to_utc())
    }
}

impl ToSql<Timestamptz, Pg> for Timestamp
where
    DateTime<Utc>: ToSql<Timestamptz, Pg>,
{
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        ToSql::<Timestamptz, Pg>::to_sql(&self.0, out)
    }
}

impl FromSql<Timestamptz, Pg> for Timestamp
where
    DateTime<Utc>: FromSql<Timestamptz, Pg>,
{
    fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
        <DateTime<Utc> as FromSql<Timestamptz, Pg>>::from_sql(bytes).map(Self)
    }
}

/// Errors that can occur when parsing or constructing a [`Timestamp`].
#[derive(Debug, Error)]
pub enum ParseError {
    /// Format of the input datetime string is invalid according to [RFC 3339].
    ///
    /// [RFC 3339]: https://www.rfc-editor.org/rfc/rfc3339
    #[error("provided value is not in a RFC 3339 format")]
    Format,

    /// Numerical timestamp value is out of the representable range.
    #[error("the value of a field is not in an allowed range")]
    Range,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_parse_valid_rfc3339_utc() {
        let input = "2026-08-02T20:56:31Z";
        let parsed = Timestamp::parse(input);
        assert!(parsed.is_ok());
        let ts = parsed.unwrap();
        assert_eq!(ts.timestamp(), 1_785_704_191);
    }

    #[test]
    fn should_parse_valid_rfc3339_with_positive_offset() {
        let input = "2026-08-02T22:56:31+02:00";
        let parsed = Timestamp::parse(input);
        assert!(parsed.is_ok());
        let ts = parsed.unwrap();
        assert_eq!(ts.timestamp(), 1_785_704_191);
    }

    #[test]
    fn should_parse_valid_rfc3339_with_negative_offset() {
        let input = "2026-08-02T15:56:31-05:00";
        let parsed = Timestamp::parse(input);
        assert!(parsed.is_ok());
        let ts = parsed.unwrap();
        assert_eq!(ts.timestamp(), 1_785_704_191);
    }

    #[test]
    fn should_parse_valid_rfc3339_with_fractional_seconds() {
        let input = "2026-08-02T20:56:31.123456Z";
        let parsed = Timestamp::parse(input);
        assert!(parsed.is_ok());
        let ts = parsed.unwrap();
        assert_eq!(ts.timestamp_millis(), 1_785_704_191_123);
    }

    #[test]
    fn should_fail_parsing_invalid_rfc3339_format() {
        let invalid_inputs = [
            "not-a-timestamp",
            "2026-08-02",
            "2026-08-02T20:56:31",
            "2026-02-31T20:56:31Z",
            "2026-13-02T20:56:31Z",
            "2026-08-02T25:56:31Z",
            "2026-08-02T20:60:31Z",
            "2026-08-02T20:56:31+25:00",
            "",
        ];

        for input in invalid_inputs {
            let res = Timestamp::parse(input);
            assert!(res.is_err(), "expected error for input: {input}");
        }
    }

    #[test]
    fn should_construct_now() {
        let ts = Timestamp::now();
        assert!(ts.timestamp() > 0);
    }

    #[test]
    fn should_convert_from_and_to_unix_secs() {
        let secs = 1_700_000_000;
        let ts = Timestamp::from_secs(secs).unwrap();
        assert_eq!(ts.timestamp(), secs);
    }

    #[test]
    fn should_convert_from_and_to_unix_millis() {
        let millis = 1_700_000_000_123;
        let ts = Timestamp::from_millis(millis).unwrap();
        assert_eq!(ts.timestamp_millis(), millis);
    }

    #[test]
    fn should_convert_from_unix_micros() {
        let micros = 1_700_000_000_123_456;
        let ts = Timestamp::from_micros(micros).unwrap();
        assert_eq!(ts.timestamp_millis(), 1_700_000_000_123);
    }

    #[test]
    fn should_fail_on_out_of_range_unix_timestamps() {
        assert!(Timestamp::from_secs(i64::MAX).is_err());
        assert!(Timestamp::from_secs(i64::MIN).is_err());
    }

    #[test]
    fn should_format_display_as_rfc3339() {
        let ts = Timestamp::from_millis(1_785_704_191_123).unwrap();
        let formatted = ts.to_string();
        assert_eq!(formatted, "2026-08-02T20:56:31.123Z");
    }

    #[test]
    fn should_convert_from_datetime_utc() {
        let dt = Utc::now();
        let ts: Timestamp = dt.into();
        let back: DateTime<Utc> = ts.into();
        assert_eq!(dt, back);
    }

    #[test]
    fn should_convert_into_naive_datetime() {
        let ts = Timestamp::from_secs(1_700_000_000).unwrap();
        let naive: NaiveDateTime = ts.into();
        assert_eq!(
            naive,
            DateTime::from_timestamp(1_700_000_000, 0)
                .unwrap()
                .naive_utc()
        );
    }

    #[test]
    fn should_implement_diesel_traits() {
        fn assert_diesel_traits<T>()
        where
            T: AsExpression<Timestamptz>
                + FromSqlRow<Timestamptz, Pg>
                + ToSql<Timestamptz, Pg>
                + FromSql<Timestamptz, Pg>,
        {
        }

        assert_diesel_traits::<Timestamp>();
    }
}
