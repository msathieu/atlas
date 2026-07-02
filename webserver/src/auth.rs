use crate::{
    JWT_KEY, get_db,
    util::{decrypt_body, sign_response, validate_timestamp},
};
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use aws_lc_rs::signature::{ED25519, ParsedPublicKey, UnparsedPublicKey};
use axum::{
    Json,
    body::Bytes,
    http::{HeaderMap, StatusCode},
};
use axum_extra::headers::{Authorization, HeaderMapExt, authorization::Bearer};
use base64::{Engine, prelude::BASE64_STANDARD};
use diesel::prelude::*;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
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
    pubkey: Option<String>,
}
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
const JWT_EXPIRY: usize = 8 * 60 * 60;
#[cfg(not(debug_assertions))]
const JWT_EXPIRY: usize = 60 * 60;

pub fn parse_pubkey(pubkey: String) -> Result<ParsedPublicKey, StatusCode> {
    let key = UnparsedPublicKey::new(&ED25519, BASE64_STANDARD.decode(pubkey).or(Err(StatusCode::BAD_REQUEST))?)
        .parse()
        .or(Err(StatusCode::BAD_REQUEST))?;
    Ok(key)
}
pub fn generate_jwt(aud: &str, user: &User, exp: usize, pubkey: Option<String>) -> Result<String, StatusCode> {
    if let Some(pubkey) = &pubkey {
        parse_pubkey(pubkey.clone())?;
    }
    let header = Header::new(Algorithm::HS512);
    let now = SystemTime::now().duration_since(UNIX_EPOCH).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?.as_secs() as usize;
    let claims = Claims {
        aud: aud.to_string(),
        sub: user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?.to_string(),
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
    let request: LoginRequest = serde_json::from_str(&decrypt_body(body.clone())?).or(Err(StatusCode::BAD_REQUEST))?;
    validate_timestamp(request.timestamp)?;
    let mut db = get_db()?;
    let user: User = users::table.filter(users::username.eq(request.username)).first(&mut db).or(Err(StatusCode::UNAUTHORIZED))?;
    let hash = PasswordHash::new(&user.password).or(Err(StatusCode::UNAUTHORIZED))?;
    Argon2::default().verify_password(request.password.as_bytes(), &hash).or(Err(StatusCode::UNAUTHORIZED))?;
    let jwt = generate_jwt("webserver", &user, JWT_EXPIRY, Some(request.pubkey))?;
    let response = LoginResponse { jwt };
    Ok((sign_response(format!("login-{}", BASE64_STANDARD.encode(Sha512::digest(body))), Some(&response))?, Json(response)))
}
pub fn authenticate_jwt(jwt: &str, aud: &str) -> Result<(User, Option<ParsedPublicKey>), StatusCode> {
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
    let mut pubkey = None;
    if let Some(pubkey_claim) = token.claims.pubkey {
        pubkey = Some(parse_pubkey(pubkey_claim)?);
    }
    Ok((user, pubkey))
}
pub fn authenticate(headers: &HeaderMap) -> Result<(User, Option<ParsedPublicKey>), StatusCode> {
    let bearer: Authorization<Bearer> = headers.typed_get().ok_or(StatusCode::UNAUTHORIZED)?;
    authenticate_jwt(bearer.token(), "webserver")
}
pub fn require_admin(headers: &HeaderMap) -> Result<(), StatusCode> {
    let user = authenticate(headers)?.0;
    if !user.admin {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(())
}
