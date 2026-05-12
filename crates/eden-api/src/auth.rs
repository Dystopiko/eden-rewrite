use eden_model::tables::tokens::PermissionScope;

#[derive(Clone, Debug)]
pub enum AuthRequirement {
    McServer,
    User { permissions: PermissionScope },
}

impl AuthRequirement {}

// #[derive(Clone, Debug)]
// pub enum AuthRequirement {
//     McServer,
//     User { permissions: PermissionScope },
// }

// use eden_api_types::error::ErrorCode;
// use eden_common::{CachedRepository, token::RawToken};
// use eden_model::tables::tokens::{PermissionScope, TokenType};
// use eden_postgres::error::QueryResultExt;

// use crate::ApiError;

// pub struct AuthCheck {
//     requirement: AuthRequirement,
// }

// const MAYBE_INVALID_TOKEN: ApiError = ApiError::from_static(
//     ErrorCode::Forbidden,
//     "This token is either invalid or expired.",
// );

// impl AuthCheck {
//     pub fn new(requirement: AuthRequirement) -> Self {
//         Self { requirement }
//     }

//     pub async fn check(
//         &self,
//         repository: &CachedRepository<'_>,
//         token: &str,
//     ) -> Result<(), ApiError> {
//         let hashed_token = RawToken::parse(token.into())
//             .ok_or(MAYBE_INVALID_TOKEN)?
//             .hash();

//         let Some(token) = repository.find_token(&hashed_token).await.optional()? else {
//             return Err(MAYBE_INVALID_TOKEN);
//         };

//         match self.requirement {
//             AuthRequirement::McServer {} if token.kind != TokenType::McServer {} => {
//                 return Err(ApiError::from_static(
//                     ErrorCode::Forbidden,
//                     "This action can only be performed for EdenMC mod.",
//                 ));
//             }
//             AuthRequirement::User {
//                 permissions: requirements,
//             } if token.kind != TokenType::User => {
//                 let permissions = token.permissions.unwrap_or_else(PermissionScope::empty);
//                 if !permissions.has(requirements) {
//                     return Err(ApiError::from_static(
//                         ErrorCode::Forbidden,
//                         "This token does not have required permissions to perform this action.",
//                     ));
//                 }

//                 return Err(ApiError::from_static(
//                     ErrorCode::Forbidden,
//                     "This action can only be performed for users.",
//                 ));
//             }
//             _ => {}
//         };

//         Ok(())
//     }
// }

// pub enum AuthRequirement {
//     McServer {},
//     User { permissions: PermissionScope },
// }

// // use eden_model::tables::tokens::PermissionScope;

// // #[derive(Clone, Debug)]
// // #[must_use]
// // pub enum AuthCheck {
// //     McServer {},
// //     User { permissions: PermissionScope },
// // }

// // impl AuthCheck {
// //     pub fn for_minecraft_server() -> Self {
// //         Self::McServer {}
// //     }

// //     pub fn for_user(permissions: PermissionScope) -> Self {
// //         Self::User { permissions }
// //     }
// // }
