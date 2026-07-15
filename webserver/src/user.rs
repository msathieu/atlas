use crate::util::{decrypt_body, sign_response, validate_signature};
use crate::{auth::authenticate, get_db};
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::Json;
use axum::body::Bytes;
use axum::extract::OriginalUri;
use axum::http::{HeaderMap, StatusCode};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use shared_server::hash_password;
use shared_server::schema::users;
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
}
pub async fn change_password(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<HeaderMap, StatusCode> {
    let user = authenticate(&headers)?.0;
    let signature = validate_signature(&headers, uri.0, &body, "POST").await?;
    let request: ChangePasswordRequest = serde_json::from_slice(&decrypt_body(body)?).or(Err(StatusCode::BAD_REQUEST))?;
    if request.new_password.is_empty() || request.new_password.len() < MIN_PASSWORD_LENGTH {
        return Err(StatusCode::BAD_REQUEST);
    }
    let hash = PasswordHash::new(&user.password).or(Err(StatusCode::FORBIDDEN))?;
    Argon2::default().verify_password(request.old_password.as_bytes(), &hash).or(Err(StatusCode::FORBIDDEN))?;
    let mut db = get_db()?;
    let hash = hash_password(request.new_password).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    diesel::update(users::table.find(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
        .set((users::password.eq(hash), users::jwt_counter.eq(user.jwt_counter + 1)))
        .execute(&mut db)
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    sign_response::<String>(signature, None)
}
pub async fn get_user_info(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<(HeaderMap, Json<GetUserInfoResponse>), StatusCode> {
    let user = authenticate(&headers)?.0;
    let signature = validate_signature(&headers, uri.0, &body, "GET").await?;
    let response = GetUserInfoResponse {
        id: user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?,
        username: user.username,
        admin: user.admin,
    };
    Ok((sign_response(signature, Some(&response))?, Json(response)))
}
