use diesel::{
    backend::Backend,
    deserialize::{FromSql, FromSqlRow},
    expression::AsExpression,
    pg::Pg,
    serialize::ToSql,
    sql_types,
};
use std::{
    fmt,
    hash::Hash,
    num::{NonZeroU64, TryFromIntError},
    ops::Deref,
};
use thiserror::Error;
use twilight_model::id::Id;

/// Database compatible type for any Discord resource IDs in [`twilight_model`].
///
/// This type automatically dereferences to its true type so it can
/// be treated like those implemented from [twilight].
///
/// [twilight]: twilight_model
#[derive(AsExpression, Clone, Copy, Eq, FromSqlRow, Ord, PartialEq, PartialOrd)]
#[diesel(sql_type = sql_types::BigInt)]
pub struct Snowflake(Id<()>);

impl Snowflake {
    /// Creates a new ID, panicking if the value is zero.
    ///
    /// Read [`Id::new`] for further documentation.
    ///
    /// # Panics
    ///
    /// Panics if the value is 0.
    #[must_use]
    #[track_caller]
    pub const fn new(n: u64) -> Self {
        Self(Id::new(n))
    }

    /// Create an ID if the provided value is not zero.
    ///
    /// Read [`Id::new_checked`] for further documentation.
    pub const fn new_checked(n: u64) -> Option<Self> {
        if let Some(id) = Id::new_checked(n) {
            Some(Self(id))
        } else {
            None
        }
    }

    /// Return the original [`Id<T>`] value.
    #[must_use]
    pub const fn into_inner<T>(self) -> Id<T> {
        self.0.cast()
    }
}

impl From<NonZeroU64> for Snowflake {
    fn from(value: NonZeroU64) -> Self {
        Self(value.into())
    }
}

impl From<Snowflake> for NonZeroU64 {
    fn from(value: Snowflake) -> Self {
        value.0.into_nonzero()
    }
}

impl<T> From<Id<T>> for Snowflake {
    fn from(value: Id<T>) -> Self {
        Self(value.cast())
    }
}

impl<T> From<Snowflake> for Id<T> {
    fn from(value: Snowflake) -> Self {
        value.0.cast()
    }
}

impl fmt::Debug for Snowflake {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Snowflake(")?;
        fmt::Debug::fmt(&self.0.get(), f)?;
        f.write_str(")")
    }
}

impl fmt::Display for Snowflake {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl Hash for Snowflake {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl PartialEq<i64> for Snowflake {
    fn eq(&self, other: &i64) -> bool {
        self.0.eq(other)
    }
}

impl<T> PartialEq<Id<T>> for Snowflake {
    fn eq(&self, other: &Id<T>) -> bool {
        self.0.cast().eq(other)
    }
}

impl PartialEq<u64> for Snowflake {
    fn eq(&self, other: &u64) -> bool {
        self.0.eq(other)
    }
}

impl PartialEq<Snowflake> for i64 {
    fn eq(&self, other: &Snowflake) -> bool {
        self.eq(&other.0)
    }
}

impl<T> PartialEq<Snowflake> for Id<T> {
    fn eq(&self, other: &Snowflake) -> bool {
        self.eq(&other.0.cast())
    }
}

impl PartialEq<Snowflake> for u64 {
    fn eq(&self, other: &Snowflake) -> bool {
        self.eq(&other.0)
    }
}

impl TryFrom<i64> for Snowflake {
    type Error = TryFromIntError;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        Id::try_from(value).map(Self)
    }
}

impl TryFrom<u64> for Snowflake {
    type Error = TryFromIntError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Id::try_from(value).map(Self)
    }
}

impl Deref for Snowflake {
    type Target = Id<()>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromSql<sql_types::BigInt, Pg> for Snowflake
where
    i64: FromSql<sql_types::BigInt, Pg>,
{
    fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        let raw_value = i64::from_sql(bytes)?;
        let snowflake = Snowflake::try_from(raw_value)?;
        Ok(snowflake)
    }
}

#[derive(Debug, Error)]
#[error("snowflake value out of bounds")]
struct OutOfBounds;

impl ToSql<sql_types::BigInt, Pg> for Snowflake
where
    i64: ToSql<sql_types::BigInt, Pg>,
{
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, Pg>,
    ) -> diesel::serialize::Result {
        let value = self.0.get();

        // Making sure that snowflake does not go past the 64-bit signed integer
        // limit. Discord limits their snowflake up to 63 bits anyways.
        if value > i64::MAX as u64 {
            return Err(Box::new(OutOfBounds));
        }

        let value: i64 = value.try_into()?;
        <i64 as ToSql<sql_types::BigInt, Pg>>::to_sql(&value, &mut out.reborrow())
    }
}
