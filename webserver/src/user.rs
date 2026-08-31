use crate::util::{decrypt_body, sign_response, validate_signature};
use crate::{auth::authenticate, get_db};
use axum::Json;
use axum::body::Bytes;
use axum::extract::OriginalUri;
use axum::http::{HeaderMap, StatusCode};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use serde::{Deserialize, Serialize};
use shared_server::schema::users;
use shared_server::{hash_password, notify_server, verify_password};
use uuid::Uuid;

#[cfg(debug_assertions)]
pub const MIN_PASSWORD_LENGTH: usize = 4;
#[cfg(not(debug_assertions))]
pub const MIN_PASSWORD_LENGTH: usize = 16;

#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    old_password: String,
    new_password: String,
}
#[derive(Serialize)]
pub struct GetUserInfoResponse {
    id: Uuid,
    username: String,
    admin: bool,
    password_enabled: bool,
    email: Option<String>,
}
#[derive(Deserialize)]
pub struct UpdateUserInfoRequest {
    email: Option<String>,
    password_state: Option<bool>,
    password: String,
}

pub async fn change_password(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<HeaderMap, StatusCode> {
    let user = authenticate(&headers).await?.0;
    let signature = validate_signature(&headers, uri.0, &body, "POST").await?;
    let request: ChangePasswordRequest = serde_json::from_slice(&decrypt_body(body)?).or(Err(StatusCode::BAD_REQUEST))?;
    if request.new_password.is_empty() || request.new_password.len() < MIN_PASSWORD_LENGTH {
        return Err(StatusCode::BAD_REQUEST);
    }
    verify_password(user.password, request.old_password).await.or(Err(StatusCode::FORBIDDEN))?;
    let mut db = get_db().await?;
    let hash = hash_password(request.new_password).await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    diesel::update(users::table.find(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
        .set((users::password.eq(hash), users::jwt_counter.eq(user.jwt_counter + 1)))
        .execute(&mut db)
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    sign_response::<String>(signature, None)
}
pub async fn get_user_info(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<(HeaderMap, Json<GetUserInfoResponse>), StatusCode> {
    let user = authenticate(&headers).await?.0;
    let signature = validate_signature(&headers, uri.0, &body, "GET").await?;
    let response = GetUserInfoResponse {
        id: user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?,
        username: user.username,
        admin: user.admin,
        password_enabled: user.password_enabled,
        email: user.email,
    };
    Ok((sign_response(signature, Some(&response))?, Json(response)))
}
pub async fn update_user_info(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<HeaderMap, StatusCode> {
    let user = authenticate(&headers).await?.0;
    let signature = validate_signature(&headers, uri.0, &body, "PATCH").await?;
    let request: UpdateUserInfoRequest = serde_json::from_slice(&decrypt_body(body)?).or(Err(StatusCode::BAD_REQUEST))?;
    verify_password(user.password, request.password).await.or(Err(StatusCode::FORBIDDEN))?;
    let mut db = get_db().await?;
    if let Some(password_state) = request.password_state {
        diesel::update(users::table.find(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
            .set(users::password_enabled.eq(password_state))
            .execute(&mut db)
            .await
            .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
        notify_server("WEB:REFRESH").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    }
    if let Some(email) = request.email {
        diesel::update(users::table.find(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
            .set(users::email.eq(email))
            .execute(&mut db)
            .await
            .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
        notify_server("WEB:REFRESH").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    }
    sign_response::<String>(signature, None)
}
