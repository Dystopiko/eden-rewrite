use crate::{discord::snowflake::Snowflake, timestamp::Timestamp};
use diesel::{Selectable, deserialize::Queryable};

/// Represents a Discord guild member record stored in Eden's database.
#[derive(Clone, Debug, Eq, PartialEq, Queryable, Selectable)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[diesel(table_name = eden_database::schema::members)]
pub struct Member {
    /// The unique Discord user ID (snowflake) of the guild member.
    pub discord_user_id: Snowflake,

    /// Timestamp when the member joined the Discord guild.
    pub joined_at: Timestamp,

    /// Member's Discord username (not nickname).
    pub name: String,

    /// Discord user ID of the member who invited this user.
    pub invited_by: Option<Snowflake>,

    /// Timestamp indicating when the member record was last updated.
    pub updated_at: Option<Timestamp>,
}
