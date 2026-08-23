use crate::{
    JWT_KEY, get_db,
    util::{decrypt_body, sign_response, validate_timestamp},
};
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use aws_lc_rs::signature::ParsedPublicKey;
use axum::{
    Json,
    body::Bytes,
    http::{HeaderMap, StatusCode},
};
use axum_extra::headers::{Authorization, HeaderMapExt, authorization::Bearer};
use base64::{Engine, prelude::BASE64_STANDARD};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
use shared_server::{Claims, authenticate_jwt, models::User, parse_pubkey, schema::users};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Semaphore;

#[derive(Deserialize)]
pub struct LoginRequest {
    username: String,
    password: String,
    pubkey: String,
    timestamp: u64,
}
#[derive(Serialize)]
pub struct LoginResponse {
    jwt: String,
}

#[cfg(debug_assertions)]
pub const JWT_EXPIRY: usize = 8 * 60 * 60;
#[cfg(not(debug_assertions))]
pub const JWT_EXPIRY: usize = 2 * 60 * 60;

static HASHING_SEMAPHORE: Semaphore = Semaphore::const_new(5);

pub fn generate_jwt(aud: &str, user: &User, exp: usize, pubkey: Option<String>) -> Result<String, StatusCode> {
    if let Some(pubkey) = &pubkey {
        parse_pubkey(pubkey.clone()).or(Err(StatusCode::BAD_REQUEST))?;
    }
    let header = Header::new(Algorithm::HS512);
    let now = SystemTime::now().duration_since(UNIX_EPOCH).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?.as_secs() as usize;
    let claims = Claims {
        aud: aud.to_string(),
        sub: user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?,
        nbf: now,
        exp: now + exp,
        counter: user.jwt_counter as usize,
        pubkey,
    };
    let key = EncodingKey::from_secret(JWT_KEY.get().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?);
    let jwt = encode(&header, &claims, &key).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(jwt)
}
pub async fn login(body: Bytes) -> Result<(HeaderMap, Json<LoginResponse>), StatusCode> {
    let request: LoginRequest = serde_json::from_slice(&decrypt_body(body.clone())?).or(Err(StatusCode::BAD_REQUEST))?;
    validate_timestamp(request.timestamp)?;
    let lock = HASHING_SEMAPHORE.acquire().await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let mut db = get_db().await?;
    let user: User = users::table
        .filter(users::username.eq(request.username).and(users::enabled.eq(true)).and(users::password_enabled.eq(true)))
        .first(&mut db)
        .await
        .or(Err(StatusCode::UNAUTHORIZED))?;
    let hash = PasswordHash::new(&user.password).or(Err(StatusCode::UNAUTHORIZED))?;
    Argon2::default().verify_password(request.password.as_bytes(), &hash).or(Err(StatusCode::UNAUTHORIZED))?;
    drop(lock);
    let jwt = generate_jwt("webserver", &user, JWT_EXPIRY, Some(request.pubkey))?;
    diesel::update(users::table.find(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
        .set(users::last_login.eq(Some(SystemTime::now())))
        .execute(&mut db)
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let response = LoginResponse { jwt };
    Ok((sign_response(format!("login-{}", BASE64_STANDARD.encode(Sha512::digest(body))), Some(&response))?, Json(response)))
}
pub async fn authenticate(headers: &HeaderMap) -> Result<(User, Option<ParsedPublicKey>), StatusCode> {
    let bearer: Authorization<Bearer> = headers.typed_get().ok_or(StatusCode::UNAUTHORIZED)?;
    let mut db = get_db().await?;
    let (user, pubkey) = authenticate_jwt(bearer.token(), "webserver", Some(&mut db), false).await.or(Err(StatusCode::UNAUTHORIZED))?;
    Ok((user.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?, pubkey))
}
pub async fn require_admin(headers: &HeaderMap) -> Result<(), StatusCode> {
    let user = authenticate(headers).await?.0;
    if !user.admin {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(())
}
