use eden_api_types::error::ErrorCode;
use eden_postgres::error::QueryResultExt;
use eden_services::repository::CachedRepository;
use uuid::Uuid;

use crate::{controllers::ApiResult, error::ApiError};

pub async fn ensure_uuid_is_not_linked(
    repository: &CachedRepository<'_>,
    uuid: Uuid,
    message: &'static str,
) -> ApiResult<()> {
    let already_linked = repository
        .find_linked_account_view(uuid)
        .await
        .optional()?
        .is_some();

    if already_linked {
        return Err(ApiError::from_static(ErrorCode::InvalidRequest, message));
    }

    Ok(())
}
