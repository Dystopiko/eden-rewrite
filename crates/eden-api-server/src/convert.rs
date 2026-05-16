use eden_model::tables::{
    linked_mc_account::LinkedMcAccount, linked_mc_account_view::LinkedMcAccountView,
    member_view::MemberFlags,
};

use self::sealed::Sealed;

pub trait LinkedMcAccountViewExt: Sealed {
    fn simplify(&self) -> LinkedMcAccount;
}

impl LinkedMcAccountViewExt for LinkedMcAccountView {
    fn simplify(&self) -> LinkedMcAccount {
        LinkedMcAccount {
            member_id: self.member.discord_user_id,
            uuid: self.uuid,
            linked_at: self.linked_at,
            username: self.username.clone(),
            edition: self.edition,
        }
    }
}

impl Sealed for LinkedMcAccountView {}

pub trait MemberFlagsExt: Sealed {
    fn api_name(&self) -> &'static str;
}

impl MemberFlagsExt for MemberFlags {
    fn api_name(&self) -> &'static str {
        match self.highest_rank() {
            Self::ADMINISTRATOR => "admin",
            Self::CONTRIBUTOR => "contributor",
            Self::STAFF => "staff",
            Self::REGULAR => "member",
            flag => {
                tracing::warn!(?flag, "unknown member flag while converting to API variant");
                "unknown"
            }
        }
    }
}

impl Sealed for MemberFlags {}

mod sealed {
    pub trait Sealed {}
}
