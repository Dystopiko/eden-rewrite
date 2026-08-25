pub mod code;
pub use self::code::{ChallengeCode, HashedChallengeCode};

use crate::timestamp::Timestamp;
use diesel::{Selectable, deserialize::Queryable};
use eden_database::enums::{ChallengeMethod, ChallengeStatus, McEdition};
use ipnet::IpNet;
use uuid::Uuid;

/// Represents an active or historical Minecraft account linking challenge session.
#[derive(Clone, Debug, Eq, PartialEq, Queryable, Selectable)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[diesel(table_name = eden_database::schema::mc_link_challenges)]
pub struct McLinkChallenge {
    /// Unique identifier for the account linking challenge session.
    pub id: Uuid,

    /// Timestamp when the challenge session was created.
    pub created_at: Timestamp,

    /// The verification method selected for this linking challenge.
    pub method: ChallengeMethod,

    /// Timestamp when this challenge expires (defaults to 10 minutes after creation).
    pub expires_at: Timestamp,

    /// Domain-separated SHA-256 hash of the challenge code.
    ///
    /// # Database Constraint (`check_hashed_code_by_method`)
    ///
    /// - Must be [`Some`]: If `method` is [`ChallengeMethod::Code`] and `status` is [`ChallengeStatus::InProgress`].
    /// - Must be [`None`]: If `method` is [`ChallengeMethod::OAuth`], or once the challenge is
    ///   completed ([`ChallengeStatus::Done`]) or cancelled ([`ChallengeStatus::Cancelled`]).
    pub hashed_code: Option<String>,

    /// Minecraft UUID of the player attempting to link their account.
    pub player_uuid: Uuid,

    /// In-game username of the player at the time the challenge was initiated.
    pub username: String,

    /// The Minecraft edition of the player (Java or Bedrock).
    pub edition: McEdition,

    /// IP address recorded when the challenge request was initiated.
    pub ip_address: IpNet,

    /// Current status of the challenge.
    pub status: ChallengeStatus,

    /// Optional timestamp indicating when the challenge status or data was last updated.
    pub updated_at: Option<Timestamp>,
}
