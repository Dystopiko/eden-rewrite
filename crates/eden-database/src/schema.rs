// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "challenge_method_enum"))]
    pub struct ChallengeMethodEnum;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "challenge_status_enum"))]
    pub struct ChallengeStatusEnum;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "mc_edition_enum"))]
    pub struct McEditionEnum;
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::ChallengeMethodEnum;
    use super::sql_types::McEditionEnum;
    use super::sql_types::ChallengeStatusEnum;

    mc_link_challenges (id) {
        id -> Uuid,
        created_at -> Timestamptz,
        method -> ChallengeMethodEnum,
        expires_at -> Timestamptz,
        #[max_length = 255]
        hashed_code -> Nullable<Varchar>,
        player_uuid -> Uuid,
        #[max_length = 20]
        username -> Varchar,
        edition -> McEditionEnum,
        ip_address -> Inet,
        status -> ChallengeStatusEnum,
        updated_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    members (discord_user_id) {
        discord_user_id -> Int8,
        joined_at -> Timestamptz,
        #[max_length = 35]
        name -> Varchar,
        invited_by -> Nullable<Int8>,
        updated_at -> Nullable<Timestamptz>,
    }
}
