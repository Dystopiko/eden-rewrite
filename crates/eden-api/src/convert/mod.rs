use eden_api_types::{
    admin::settings::EncodedSettings,
    types::{EncodedMember, FullMcAccount, FullMember},
};
use eden_model::tables::{
    linked_mc_account_view::LinkedMcAccountView, member_view::MemberView, settings::Settings,
};
use eden_services::ext::MemberFlagsExt;

pub fn into_encoded_settings(settings: Settings) -> EncodedSettings {
    EncodedSettings {
        allow_guests: settings.allow_guests,
        updated_at: settings.updated_at.unwrap_or(settings.created_at),
    }
}

pub fn into_full_mc_account(account: LinkedMcAccountView) -> FullMcAccount {
    FullMcAccount {
        uuid: account.uuid.hyphenated(),
        username: account.username,
        edition: account.edition,
        linked_at: account.linked_at,
        last_login_at: None,
        last_ip_address: None,
    }
}

pub fn into_full_member(member: MemberView, accounts: Vec<LinkedMcAccountView>) -> FullMember {
    FullMember {
        id: member.discord_user_id.cast(),
        name: member.name,
        rank: member.flags.api_name().to_string(),
        invited_by: member.inviter.map(|inviter| EncodedMember {
            id: inviter.discord_user_id.cast(),
            name: inviter.name,
            rank: Some(inviter.flags.api_name().to_string()),
            status: None,
            last_login_at: None,
        }),
        accounts: accounts.into_iter().map(into_full_mc_account).collect(),
    }
}
