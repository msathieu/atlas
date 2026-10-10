use crate::auth::require_admin;
use crate::get_db;
use crate::user::MIN_PASSWORD_LENGTH;
use crate::util::{convert_timestamp, decrypt_body, sign_response, validate_signature};
use axum::Json;
use axum::body::Bytes;
use axum::extract::{OriginalUri, Path};
use axum::http::{HeaderMap, StatusCode};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use serde::{Deserialize, Serialize};
use shared_server::models::User;
use shared_server::schema::users;
use shared_server::{hash_password, notify_server, verify_password};
use uuid::Uuid;

#[derive(Serialize)]
pub struct APIUser {
    id: Uuid,
    username: String,
    admin: bool,
    enabled: bool,
    password_enabled: bool,
    last_login: Option<u64>,
    email: String,
}
#[derive(Deserialize)]
pub struct CreateUserRequest {
    username: String,
    email: String,
    password: String,
    admin: bool,
    auth_password: String,
}
#[derive(Deserialize)]
pub struct DeleteUserRequest {
    password: String,
}
#[derive(Deserialize)]
pub struct UpdateUserRequest {
    admin: bool,
    password: String,
}
#[derive(Serialize)]
pub struct CreateUserResponse {
    id: Uuid,
}

pub async fn users_list(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<(HeaderMap, Json<Vec<APIUser>>), StatusCode> {
    require_admin(&headers).await?;
    let signature = validate_signature(&headers, uri.0, &body, "GET").await?;
    let mut db = get_db().await?;
    let users: Vec<User> = users::table.get_results(&mut db).await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let users: Result<Vec<APIUser>, StatusCode> = users
        .iter()
        .map(|user| {
            Ok(APIUser {
                id: user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?,
                username: user.username.clone(),
                admin: user.admin,
                enabled: user.enabled,
                password_enabled: user.password_enabled,
                last_login: user.last_login.map(|timestamp| convert_timestamp(timestamp).unwrap_or(0)),
                email: user.email.clone(),
            })
        })
        .collect();
    let users = users?;
    Ok((sign_response(signature, Some(&users))?, Json(users)))
}
pub async fn delete_user(headers: HeaderMap, Path(id): Path<Uuid>, uri: OriginalUri, body: Bytes) -> Result<HeaderMap, StatusCode> {
    let user = require_admin(&headers).await?;
    let signature = validate_signature(&headers, uri.0, &body, "DELETE").await?;
    let request: DeleteUserRequest = serde_json::from_slice(&decrypt_body(body)?).or(Err(StatusCode::BAD_REQUEST))?;
    verify_password(user.password, request.password).await.or(Err(StatusCode::FORBIDDEN))?;
    let mut db = get_db().await?;
    diesel::delete(users::table.find(id)).execute(&mut db).await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server("WEB:REFRESH").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    sign_response::<String>(signature, None)
}
pub async fn update_user(headers: HeaderMap, Path(id): Path<Uuid>, uri: OriginalUri, body: Bytes) -> Result<HeaderMap, StatusCode> {
    let user = require_admin(&headers).await?;
    let signature = validate_signature(&headers, uri.0, &body, "PATCH").await?;
    let request: UpdateUserRequest = serde_json::from_slice(&decrypt_body(body)?).or(Err(StatusCode::BAD_REQUEST))?;
    verify_password(user.password, request.password).await.or(Err(StatusCode::FORBIDDEN))?;
    let mut db = get_db().await?;
    diesel::update(users::table.find(id))
        .set(users::admin.eq(request.admin))
        .execute(&mut db)
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server("WEB:REFRESH").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    sign_response::<String>(signature, None)
}
pub async fn create_user(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<(HeaderMap, Json<CreateUserResponse>), StatusCode> {
    let user = require_admin(&headers).await?;
    let signature = validate_signature(&headers, uri.0, &body, "POST").await?;
    let request: CreateUserRequest = serde_json::from_slice(&decrypt_body(body)?).or(Err(StatusCode::BAD_REQUEST))?;
    verify_password(user.password, request.auth_password).await.or(Err(StatusCode::FORBIDDEN))?;
    if request.username.is_empty()
        || !request.username.chars().all(|c| c.is_ascii_alphanumeric())
        || request.email.trim().is_empty()
        || request.password.is_empty()
        || request.password.len() < MIN_PASSWORD_LENGTH
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut db = get_db().await?;
    let hash = hash_password(request.password).await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let mut user = User {
        id: None,
        username: request.username,
        password: hash,
        jwt_counter: 0,
        admin: request.admin,
        enabled: true,
        password_enabled: true,
        last_login: None,
        email: request.email,
    };
    user = diesel::insert_into(users::table).values(user).get_result(&mut db).await.or(Err(StatusCode::BAD_REQUEST))?;
    notify_server("WEB:REFRESH").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let response = CreateUserResponse {
        id: user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?,
    };
    Ok((sign_response(signature, Some(&response))?, Json(response)))
}
