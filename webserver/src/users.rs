use crate::auth::require_admin;
use crate::get_db;
use crate::user::MIN_PASSWORD_LENGTH;
use crate::util::{decrypt_body, sign_response, validate_signature};
use axum::Json;
use axum::body::Bytes;
use axum::extract::{OriginalUri, Path};
use axum::http::{HeaderMap, StatusCode};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use shared_server::models::User;
use shared_server::schema::users;
use shared_server::{hash_password, notify_server};
use uuid::Uuid;

#[derive(Serialize)]
pub struct APIUser {
    id: Uuid,
    username: String,
    admin: bool,
}
#[derive(Deserialize)]
pub struct CreateUserRequest {
    username: String,
    password: String,
    admin: bool,
}
#[derive(Serialize)]
pub struct CreateUserResponse {
    id: Uuid,
}

pub async fn users_list(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<(HeaderMap, Json<Vec<APIUser>>), StatusCode> {
    require_admin(&headers)?;
    let signature = validate_signature(&headers, uri.0, &body, "GET").await?;
    let mut db = get_db()?;
    let users: Vec<User> = users::table.get_results(&mut db).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let users: Result<Vec<APIUser>, StatusCode> = users
        .iter()
        .map(|user| {
            Ok(APIUser {
                id: user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?,
                username: user.username.clone(),
                admin: user.admin,
            })
        })
        .collect();
    let users = users?;
    Ok((sign_response(signature, Some(&users))?, Json(users)))
}
pub async fn delete_user(headers: HeaderMap, Path(id): Path<String>, uri: OriginalUri, body: Bytes) -> Result<HeaderMap, StatusCode> {
    require_admin(&headers)?;
    let signature = validate_signature(&headers, uri.0, &body, "DELETE").await?;
    let mut db = get_db()?;
    diesel::delete(users::table.find(Uuid::parse_str(&id).or(Err(StatusCode::BAD_REQUEST))?))
        .execute(&mut db)
        .or(Err(StatusCode::NOT_FOUND))?;
    notify_server("WEB:REFRESH").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    sign_response::<String>(signature, None)
}
pub async fn create_user(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<(HeaderMap, Json<CreateUserResponse>), StatusCode> {
    require_admin(&headers)?;
    let signature = validate_signature(&headers, uri.0, &body, "POST").await?;
    let request: CreateUserRequest = serde_json::from_str(&decrypt_body(body).or(Err(StatusCode::BAD_REQUEST))?).or(Err(StatusCode::BAD_REQUEST))?;
    if request.username.is_empty() || request.password.is_empty() || request.password.len() < MIN_PASSWORD_LENGTH {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut db = get_db()?;
    let hash = hash_password(request.password).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let mut user = User {
        id: None,
        username: request.username,
        password: hash,
        jwt_counter: 0,
        admin: request.admin,
    };
    user = diesel::insert_into(users::table).values(user).get_result(&mut db).or(Err(StatusCode::BAD_REQUEST))?;
    notify_server("WEB:REFRESH").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let response = CreateUserResponse {
        id: user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?,
    };
    Ok((sign_response(signature, Some(&response))?, Json(response)))
}
