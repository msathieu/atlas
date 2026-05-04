use crate::util::decrypt_body;
use crate::{auth::authenticate, get_db};
use argon2::PasswordHasher;
use argon2::{
    Algorithm, Argon2, Params, PasswordHash, PasswordVerifier, Version,
    password_hash::{SaltString, rand_core::OsRng},
};
use axum::body::Bytes;
use axum::http::{HeaderMap, StatusCode};
use diesel::prelude::*;
use serde::Deserialize;
use shared_server::schema::users;

#[cfg(debug_assertions)]
const MIN_PASSWORD_LENGTH: usize = 4;
#[cfg(not(debug_assertions))]
const MIN_PASSWORD_LENGTH: usize = 16;

#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    old_password: String,
    new_password: String,
}

pub async fn change_password(headers: HeaderMap, body: Bytes) -> Result<(), StatusCode> {
    let user = authenticate(headers).await?;
    let request: ChangePasswordRequest = serde_json::from_str(&decrypt_body(body)?).or(Err(StatusCode::BAD_REQUEST))?;
    if request.new_password.is_empty() || request.new_password.len() < MIN_PASSWORD_LENGTH {
        return Err(StatusCode::BAD_REQUEST);
    }
    let hash = PasswordHash::new(&user.password).or(Err(StatusCode::FORBIDDEN))?;
    Argon2::default().verify_password(request.old_password.as_bytes(), &hash).or(Err(StatusCode::FORBIDDEN))?;
    let mut db = get_db()?;
    let params = Params::new(128 * 1024, 1, 1, Some(64)).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let salt = SaltString::generate(OsRng);
    let hash = argon.hash_password(request.new_password.as_bytes(), &salt).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    diesel::update(users::table.find(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
        .set((users::password.eq(hash.to_string()), users::jwt_counter.eq(user.jwt_counter + 1)))
        .execute(&mut db)
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(())
}
