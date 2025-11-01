use crate::{JWT_KEY, get_db};
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::{
    Json,
    http::{HeaderMap, StatusCode},
};
use axum_extra::headers::{Authorization, HeaderMapExt, authorization::Bearer};
use diesel::prelude::*;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use shared_server::{models::User, schema::users};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Clone, Deserialize, Serialize)]
struct Claims {
    aud: String,
    sub: String,
    nbf: usize,
    exp: usize,
    counter: usize,
}
#[derive(Deserialize)]
pub struct LoginRequest {
    username: String,
    password: String,
}
#[derive(Serialize)]
pub struct LoginResponse {
    jwt: String,
    admin: bool,
}

#[cfg(debug_assertions)]
const JWT_EXPIRY: usize = 8 * 60 * 60;
#[cfg(not(debug_assertions))]
const JWT_EXPIRY: usize = 60 * 60;

pub async fn generate_jwt(aud: &str, user: &User, exp: usize) -> Result<String, StatusCode> {
    let header = Header::new(Algorithm::HS512);
    let now = SystemTime::now().duration_since(UNIX_EPOCH).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?.as_secs() as usize;
    let claims = Claims {
        aud: aud.to_string(),
        sub: user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?.to_string(),
        nbf: now,
        exp: now + exp,
        counter: user.jwt_counter as usize,
    };
    let key = EncodingKey::from_secret(JWT_KEY.get().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?);
    let jwt = encode(&header, &claims, &key).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(jwt)
}
pub async fn login(Json(request): Json<LoginRequest>) -> Result<Json<LoginResponse>, StatusCode> {
    let mut db = get_db()?;
    let user: User = users::table.filter(users::username.eq(request.username)).first(&mut db).or(Err(StatusCode::UNAUTHORIZED))?;
    let hash = PasswordHash::new(&user.password).or(Err(StatusCode::UNAUTHORIZED))?;
    Argon2::default().verify_password(request.password.as_bytes(), &hash).or(Err(StatusCode::UNAUTHORIZED))?;
    let jwt = generate_jwt("webserver", &user, JWT_EXPIRY).await?;
    let response = LoginResponse { jwt, admin: user.admin };
    Ok(Json(response))
}
pub async fn authenticate_jwt(jwt: &str, aud: &str) -> Result<User, StatusCode> {
    let key = DecodingKey::from_secret(JWT_KEY.get().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?);
    let mut validation = Validation::new(Algorithm::HS512);
    validation.validate_nbf = true;
    validation.set_audience(&[aud]);
    validation.set_required_spec_claims(&["aud", "sub", "nbf", "exp", "counter"]);
    let token = decode::<Claims>(jwt, &key, &validation).or(Err(StatusCode::UNAUTHORIZED))?;
    let mut db = get_db()?;
    let user: User = users::table
        .find(Uuid::parse_str(&token.claims.sub).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?)
        .first(&mut db)
        .or(Err(StatusCode::UNAUTHORIZED))?;
    if user.jwt_counter as usize != token.claims.counter {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(user)
}
pub async fn authenticate(headers: HeaderMap) -> Result<User, StatusCode> {
    let bearer: Authorization<Bearer> = headers.typed_get().ok_or(StatusCode::UNAUTHORIZED)?;
    authenticate_jwt(bearer.token(), "webserver").await
}
pub async fn require_admin(headers: HeaderMap) -> Result<(), StatusCode> {
    let user = authenticate(headers).await?;
    if !user.admin {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(())
}
