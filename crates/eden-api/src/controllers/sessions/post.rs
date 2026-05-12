// use axum::extract::State;
// use eden_model::tables::tokens::PermissionScope;
// use std::sync::Arc;

// use crate::{
//     WebContext,
//     auth::AuthRequirement,
//     extract::{HasGranted, RequirementLogic},
// };

// async fn post(ctx: State<Arc<WebContext>>, _handler: HasGranted<GrantSessionRequirements>) {}

// struct GrantSessionRequirements;

// impl RequirementLogic for GrantSessionRequirements {
//     const REQUIREMENT: AuthRequirement = AuthRequirement::User {
//         permission: PermissionScope::all(),
//     };
// }

// // // use eden_model::tables::tokens::PermissionScope;

// // // use crate::{
// // //     ApiError, WebContext,
// // //     auth::{AuthCheck, AuthRequirement},
// // // };

// // use eden_model::tables::tokens::PermissionScope;

// // use crate::extract::Requires;

// // const REQS: u64 = PermissionScope::EDIT_SETTINGS
// //     .union(PermissionScope::LINK_MINECRAFT_ACCOUNTS)
// //     .bits();

// // async fn wow(_auth: Requires<REQS>) {}

// // // async fn post(ctx: WebContext) -> Result<(), ApiError> {
// // //     let repository = ctx.repository();

// // //     AuthCheck::new(AuthRequirement::User {
// // //         permissions: PermissionScope::all(),
// // //     })
// // //     .check(&repository, "hello")
// // //     .await?;

// // //     Ok(())
// // // }

// // // // use eden_api_types::sessions::RequestSession;
// // // // use eden_common::domain::notifier::LoginMetadata;
// // // // use eden_jobs::{events::OnPlayerJoinedJob, notification::NotifyPendingLoginJob};
// // // // use eden_model::{
// // // //     common::ApprovalStatus,
// // // //     tables::{linked_mc_account_view::LinkedMcAccountView, mc_login_event::NewMcLoginEvent},
// // // // };
// // // // use eden_postgres::error::QueryResultExt;

// // // // use crate::{WebContext, convert::LinkedMcAccountViewExt};

// // // // pub async fn post(ctx: WebContext, body: RequestSession) {
// // // //     let repository = ctx.repository();
// // // //     let Some(account) = repository
// // // //         .find_linked_mc_account_view(body.uuid.into_uuid())
// // // //         .await
// // // //         .optional()
// // // //         .unwrap()
// // // //     else {
// // // //         let settings = repository.settings(&ctx.config()).await.unwrap();

// // // //         todo!()
// // // //     };

// // // //     let metadata = LoginMetadata {
// // // //         edition: body.edition,
// // // //         member_id,
// // // //         ip: body.ip,
// // // //         issued_at: result.value.created_at,
// // // //         username: account.username.clone(),
// // // //         uuid: account.uuid,
// // // //     };

// // // //     validate_login(&ctx, &account, &body).await;

// // // //     let member_id = account.discord_user_id.cast();
// // // //     let perks = ctx
// // // //         .minecraft()
// // // //         .resolve_perks(account.flags, Some(member_id), Some(account.uuid));

// // // //     notify_pending_login(&ctx, metadata)
// // // // }

// // // // async fn notify_player_logged_in(
// // // //     ctx: &WebContext,
// // // //     account: &LinkedMcAccountView,
// // // //     body: &RequestSession,
// // // // ) {
// // // //     let event = NewMcLoginEvent::from_linked(&account.simplify())
// // // //         .ip_address(body.ip)
// // // //         .build();

// // // //     if let Err(error) = ctx.job_queue().enqueue_job(OnPlayerJoinedJob(event)).await {
// // // //         tracing::warn!(?error, "failed to enqueue OnPlayerJoinedJob job");
// // // //     }
// // // // }

// // // // async fn notify_pending_login(ctx: &WebContext, metadata: LoginMetadata) {
// // // //     let job = NotifyPendingLoginJob(metadata);
// // // //     if let Err(error) = ctx.job_queue().enqueue_job(job).await {
// // // //         tracing::warn!(
// // // //             ?error,
// // // //             "failed to notify the member about unrecognized ip login"
// // // //         );
// // // //     }
// // // // }

// // // // async fn validate_login(ctx: &WebContext, account: &LinkedMcAccountView, body: &RequestSession) {
// // // //     let member_id = account.member.discord_user_id.cast();
// // // //     let result = ctx
// // // //         .repository()
// // // //         .resolve_member_cidr_trust(member_id, body.ip)
// // // //         .await
// // // //         .unwrap();

// // // //     match result.value.status {
// // // //         ApprovalStatus::Approved => {}
// // // //         ApprovalStatus::Pending => {
// // // //             if result.created {
// // // //                 notify_pending_login(ctx, metadata).await;
// // // //             }
// // // //         }
// // // //         ApprovalStatus::Revoked => {
// // // //             ctx.notifier().revoked_login(&metadata).await.unwrap();
// // // //         }
// // // //     }
// // // // }
