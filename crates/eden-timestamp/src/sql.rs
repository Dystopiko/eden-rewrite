use chrono::DateTime;

use crate::Timestamp;

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
