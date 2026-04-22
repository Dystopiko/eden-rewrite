use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use error_stack::{Report, ResultExt};
use serde::{Deserialize, Serialize};
use std::{fmt::Display, str::FromStr, time::Duration};
use thiserror::Error;

/// A database-compatible type represents date and time in UTC, wrapped with
/// [`chrono::DateTime<Utc>`] internally.
///
/// # Formatting
/// Eden API timestamps are formatted as prescribed from [RFC 3339] or
/// `YYYY-MM-DDTHH:MM:SS.SSS+00:00` or any time zone available.
///
/// [RFC 3339]: https://www.rfc-editor.org/rfc/rfc3339
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(DateTime<Utc>);

#[derive(Debug, Error)]
pub enum TimestampParseError {
    /// Format of the input datetime is invalid and not prescribed from [RFC 3339].
    ///
    /// [RFC 3339]: https://www.rfc-editor.org/rfc/rfc3339
    #[error("provided value is not in a RFC 3339 format")]
    Format,

    /// Value of a field is not in an acceptable range.
    #[error("the value of a field is not in an allowed range")]
    Range,
}

impl Timestamp {
    /// Creates a new [`Timestamp`] with the current system date
    /// and time in UTC.
    #[must_use]
    pub fn now() -> Self {
        Self(Utc::now())
    }

    /// Converts [twilight's own `Timestamp` object] into [`Timestamp`].
    ///
    /// [twilight's own `Timestamp` object]: twilight_model::util::Timestamp
    #[must_use]
    pub fn from_twilight(ts: twilight_model::util::Timestamp) -> Self {
        Timestamp::from_str(&ts.iso_8601().to_string())
            .expect("twilight should emit correct ISO 8601")
    }

    /// Converts [`Timestamp`] into [twilight's own `Timestamp` object].
    ///
    /// [twilight's own `Timestamp` object]: twilight_model::util::Timestamp
    #[must_use]
    pub fn into_twilight(self) -> twilight_model::util::Timestamp {
        use twilight_model::util::Timestamp;
        Timestamp::from_str(&self.0.to_rfc3339_opts(chrono::SecondsFormat::Millis, false))
            .expect("should be compilant with Twilight's timestamp format")
    }

    /// Parses a timestamp from an RFC 3339 date and time string.
    pub fn parse(input: &str) -> Result<Self, Report<TimestampParseError>> {
        DateTime::parse_from_rfc3339(input)
            .map(|v| Self(v.to_utc()))
            .change_context(TimestampParseError::Format)
    }

    /// Creates a new [`Timestamp`] from the number of seconds since
    /// the Unix epoch (January 1st, 1970 at 00:00:00 UTC)
    pub fn from_secs(secs: i64) -> Result<Self, Report<TimestampParseError>> {
        Utc.timestamp_opt(secs, 0)
            .single()
            .map(Self)
            .ok_or_else(|| Report::new(TimestampParseError::Range))
    }

    /// Creates a new [`Timestamp`] from the number of milliseconds since
    /// the Unix epoch (January 1st, 1970 at 00:00:00 UTC)
    pub fn from_millis(millis: i64) -> Result<Self, Report<TimestampParseError>> {
        Utc.timestamp_millis_opt(millis)
            .single()
            .map(Self)
            .ok_or_else(|| Report::new(TimestampParseError::Range))
    }

    /// Creates a new [`Timestamp`] from the number of microseconds since
    /// the Unix epoch (January 1st, 1970 at 00:00:00 UTC)
    pub fn from_micros(millis: i64) -> Result<Self, Report<TimestampParseError>> {
        Utc.timestamp_micros(millis)
            .single()
            .map(Self)
            .ok_or_else(|| Report::new(TimestampParseError::Range))
    }
}

impl Timestamp {
    /// Returns the elapsed [duration] since the provided timestamp.
    ///
    /// It returns two types, the duration elaped since the provided
    /// timestamp, and whether it goes forward or backwards.
    ///
    /// It goes backward if the provided timestamp returns out to be
    /// later than the current system time, the second type will return
    /// `true`, otherwise it will return `false`.
    #[must_use]
    pub fn elapsed(&self) -> (Duration, bool) {
        let elapsed = self.0.signed_duration_since(Utc::now());
        let delta = elapsed
            .abs()
            .to_std()
            .expect("should provide std duration from non-negative time delta");

        (delta, elapsed.to_std().is_ok())
    }

    /// Returns the elapsed [duration] since the Unix epoch
    /// (January 1st, 1970 at 00:00:00 UTC) based on the provided timestamp.
    ///
    /// If the provided timestamp returns out to be earlier than the Unix epoch,
    /// it will return [`None`], but most of the cases, it will return [`Some`].
    ///
    /// [duration]: Duration
    #[must_use]
    pub fn elapsed_from_unix(&self) -> Option<Duration> {
        self.0
            .signed_duration_since(DateTime::UNIX_EPOCH)
            .to_std()
            .ok()
    }

    /// Returns the number of seconds since the Unix epoch
    /// (January 1st, 1970 at 00:00:00 UTC).
    #[must_use]
    pub const fn timestamp(&self) -> i64 {
        self.0.timestamp()
    }

    /// Returns the number of milliseconds since the Unix epoch
    /// (January 1st, 1970 at 00:00:00 UTC).
    #[must_use]
    pub const fn timestamp_millis(&self) -> i64 {
        self.0.timestamp_millis()
    }
}

impl FromStr for Timestamp {
    type Err = Report<TimestampParseError>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

struct TimestampVisitor;

impl serde::de::Visitor<'_> for TimestampVisitor {
    type Value = Timestamp;

    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Eden timestamp")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Timestamp::parse(v).map_err(serde::de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(TimestampVisitor)
    }
}

impl Serialize for Timestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(self)
    }
}

impl Display for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let rfc_3339 = self.0.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        Display::fmt(&rfc_3339, f)
    }
}

impl<Tz: chrono::TimeZone> From<DateTime<Tz>> for Timestamp {
    fn from(value: DateTime<Tz>) -> Self {
        Self(value.to_utc())
    }
}

impl From<Timestamp> for DateTime<Utc> {
    fn from(value: Timestamp) -> Self {
        value.0
    }
}

impl From<Timestamp> for NaiveDateTime {
    fn from(value: Timestamp) -> Self {
        value.0.naive_utc()
    }
}

impl<'row> sqlx::Decode<'row, sqlx::Postgres> for Timestamp
where
    DateTime<chrono::Utc>: sqlx::Decode<'row, sqlx::Postgres>,
{
    fn decode(value: sqlx::postgres::PgValueRef<'row>) -> Result<Self, sqlx::error::BoxDynError> {
        Ok(Self(DateTime::<chrono::Utc>::decode(value)?))
    }
}

impl<'query> sqlx::Encode<'query, sqlx::Postgres> for Timestamp
where
    DateTime<chrono::Utc>: sqlx::Encode<'query, sqlx::Postgres>,
{
    fn encode_by_ref(
        &self,
        buf: &mut <sqlx::Postgres as sqlx::Database>::ArgumentBuffer<'query>,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        self.0.encode(buf)
    }
}

impl sqlx::Type<sqlx::Postgres> for Timestamp
where
    DateTime<chrono::Utc>: sqlx::Type<sqlx::Postgres>,
{
    fn type_info() -> <sqlx::Postgres as sqlx::Database>::TypeInfo {
        <DateTime<chrono::Utc> as sqlx::Type<sqlx::Postgres>>::type_info()
    }
}

#[cfg(test)]
mod tests;
