use crate::{
    auth::authenticate,
    util::{decrypt_body, validate_signature},
};
use crate::{get_db, util::sign_response};
use axum::{
    body::Bytes,
    extract::{OriginalUri, Path},
    http::HeaderMap,
};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use reqwest::StatusCode;
use serde::Deserialize;
use shared_server::{notify_server, schema::containers};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct UpdateContainerRequest {
    memory_limit: Option<i32>,
    storage_size: Option<i32>,
}

pub async fn update_container(headers: HeaderMap, Path(id): Path<Uuid>, uri: OriginalUri, body: Bytes) -> Result<HeaderMap, StatusCode> {
    let user = authenticate(&headers).await?.0;
    let signature = validate_signature(&headers, uri.0, &body, "PATCH").await?;
    let request: UpdateContainerRequest = serde_json::from_slice(&decrypt_body(&headers, body)?).or(Err(StatusCode::BAD_REQUEST))?;
    let mut db = get_db().await?;
    if let Some(memory_limit) = request.memory_limit {
        if memory_limit <= 0 || memory_limit > 32 {
            return Err(StatusCode::BAD_REQUEST);
        }
        diesel::update(containers::table.filter(containers::id.eq(id).and(containers::uid.eq(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))))
            .set(containers::memory_limit.eq(memory_limit))
            .execute(&mut db)
            .await
            .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
        notify_server(&format!("WEB:REFRESH:{}", user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
            .await
            .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    }
    if let Some(storage_size) = request.storage_size {
        if storage_size <= 0 || storage_size > 256 {
            return Err(StatusCode::BAD_REQUEST);
        }
        diesel::update(
            containers::table.filter(
                containers::id
                    .eq(id)
                    .and(containers::uid.eq(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
                    .and(containers::storage_size.le(storage_size)),
            ),
        )
        .set(containers::storage_size.eq(storage_size))
        .execute(&mut db)
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
        notify_server(&format!("WEB:REFRESH:{}", user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
            .await
            .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    }
    sign_response::<String>(signature, None)
}
