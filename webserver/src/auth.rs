use crate::{
    get_db,
    util::{decrypt_body, sign_response, validate_signature, validate_timestamp},
};
use axum::{
    Json,
    body::Bytes,
    extract::OriginalUri,
    http::{HeaderMap, StatusCode},
};
use axum_extra::headers::{Authorization, HeaderMapExt, authorization::Bearer};
use base64::{Engine, prelude::BASE64_STANDARD};
use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
use shared_server::{Claims, authenticate_jwt, get_jwt_key, models::User, parse_pubkey, schema::users, verify_password};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

pub async fn generate_jwt(aud: &str, user: &User, exp: usize, pubkey: Option<String>, iat: Option<usize>, db: &mut AsyncPgConnection) -> Result<String, StatusCode> {
    if let Some(pubkey) = &pubkey {
        parse_pubkey(pubkey.clone()).or(Err(StatusCode::BAD_REQUEST))?;
    }
    let header = Header::new(Algorithm::HS512);
    let now = SystemTime::now().duration_since(UNIX_EPOCH).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?.as_secs() as usize;
    let iat = iat.unwrap_or(now);
    let claims = Claims {
        aud: aud.to_string(),
        sub: user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?,
        nbf: now,
        exp: now + exp,
        iat,
        counter: user.jwt_counter as usize,
        pubkey,
    };
    let key = EncodingKey::from_secret(&get_jwt_key(db).await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?);
    let jwt = encode(&header, &claims, &key).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(jwt)
}
pub async fn login(body: Bytes) -> Result<(HeaderMap, Json<LoginResponse>), StatusCode> {
    let request: LoginRequest = serde_json::from_slice(&decrypt_body(body.clone())?).or(Err(StatusCode::BAD_REQUEST))?;
    validate_timestamp(request.timestamp)?;
    let mut db = get_db().await?;
    let user: User = users::table
        .filter(users::username.eq(request.username).and(users::enabled.eq(true)).and(users::password_enabled.eq(true)))
        .first(&mut db)
        .await
        .or(Err(StatusCode::UNAUTHORIZED))?;
    verify_password(user.password.clone(), request.password).await.or(Err(StatusCode::UNAUTHORIZED))?;
    let jwt = generate_jwt("webserver", &user, JWT_EXPIRY, Some(request.pubkey), None, &mut db).await?;
    diesel::update(users::table.find(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
        .set(users::last_login.eq(Some(SystemTime::now())))
        .execute(&mut db)
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let response = LoginResponse { jwt };
    Ok((sign_response(format!("login-{}", BASE64_STANDARD.encode(Sha512::digest(body))), Some(&response))?, Json(response)))
}
pub async fn authenticate(headers: &HeaderMap) -> Result<(User, Option<String>, usize), StatusCode> {
    let bearer: Authorization<Bearer> = headers.typed_get().ok_or(StatusCode::UNAUTHORIZED)?;
    let mut db = get_db().await?;
    let (user, pubkey, iat) = authenticate_jwt(bearer.token(), "webserver", &mut db).await.or(Err(StatusCode::UNAUTHORIZED))?;
    Ok((user, pubkey, iat))
}
pub async fn require_admin(headers: &HeaderMap) -> Result<(), StatusCode> {
    let user = authenticate(headers).await?.0;
    if !user.admin {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(())
}
pub async fn refresh_jwt(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<(HeaderMap, Json<LoginResponse>), StatusCode> {
    let (user, pubkey, iat) = authenticate(&headers).await?;
    let signature = validate_signature(&headers, uri.0, &body, "POST").await?;
    let now = SystemTime::now();
    let parsed_iat = UNIX_EPOCH + Duration::from_secs(iat as u64);
    if now < parsed_iat || now > parsed_iat + Duration::from_hours(48) {
        return Err(StatusCode::FORBIDDEN);
    }
    let mut db = get_db().await?;
    let jwt = generate_jwt("webserver", &user, JWT_EXPIRY, pubkey, Some(iat), &mut db).await?;
    let response = LoginResponse { jwt };
    Ok((sign_response(signature, Some(&response))?, Json(response)))
}
