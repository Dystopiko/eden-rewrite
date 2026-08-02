use diesel_derive_enum::DbEnum;

/// The verification mechanism used to complete the account link challenge.
#[derive(Clone, Copy, DbEnum, Debug, Eq, PartialEq)]
#[ExistingTypePath = "crate::schema::sql_types::ChallengeMethodEnum"]
#[DbValueStyle = "verbatim"]
pub enum ChallengeMethod {
    /// OAuth2 verification flow.
    ///
    /// This is reserved for Java Edition players and Bedrock users
    /// who choose `Web Link` as their preferred linking method.
    ///
    /// If this is set, `mc_link_challenge`'s `hashed_code` must be null.
    OAuth,

    /// Color challenge code verification flow.
    ///
    /// This is for Bedrock users who choose `Link by code` as their
    /// preferred linking method.
    Code,
}

/// The current lifecycle status of an account linking challenge.
#[derive(Clone, Copy, DbEnum, Debug, Default, Eq, PartialEq)]
#[ExistingTypePath = "crate::schema::sql_types::ChallengeStatusEnum"]
#[DbValueStyle = "verbatim"]
pub enum ChallengeStatus {
    /// The challenge was manually/automatically cancelled or
    /// superseded by a newer request.
    Cancelled,

    /// The challenge is active and awaiting verification.
    #[default]
    InProgress,

    /// The challenge completed successfully and the
    /// account has been linked.
    Done,
}

/// The Minecraft client edition of a player account.
#[derive(Clone, Copy, DbEnum, Debug, Eq, Hash, PartialEq)]
#[ExistingTypePath = "crate::schema::sql_types::McEditionEnum"]
#[DbValueStyle = "verbatim"]
pub enum McEdition {
    /// This variant specifies Java edition.
    Java,

    /// This variant specifies Bedrock edition.
    Bedrock,
}
