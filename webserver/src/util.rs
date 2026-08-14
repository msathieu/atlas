use crate::{ENCRYPTION_KEY, SIGNING_KEY, auth::authenticate};
use aws_lc_rs::{
    rsa::{OAEP_SHA512_MGF1SHA512, OaepPrivateDecryptingKey, PrivateDecryptingKey},
    signature::Ed25519KeyPair,
};
use axum::{
    body::Bytes,
    http::{HeaderMap, HeaderValue, StatusCode, Uri},
};
use base64::{Engine, prelude::BASE64_STANDARD};
use redis::{AsyncTypedCommands, ExistenceCheck, SetExpiry, SetOptions};
use serde::Serialize;
use sha2::{Digest, Sha512};
use shared_server::get_valkey_client;
use std::{
    sync::OnceLock,
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

static VALKEY_POOL: OnceLock<redis::aio::ConnectionManager> = OnceLock::new();

async fn validate_nonce(nonce: Uuid) -> Result<(), StatusCode> {
    if VALKEY_POOL.get().is_none() {
        let client = get_valkey_client().await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
        VALKEY_POOL.set(client.0).ok();
    }
    let key = format!("NONCE:{}", nonce);
    let exists = VALKEY_POOL
        .get()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?
        .clone()
        .set_options(key, true, SetOptions::default().conditional_set(ExistenceCheck::NX).with_expiration(SetExpiry::EX(60)))
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    if exists.is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(())
}
pub fn convert_timestamp(time: SystemTime) -> Result<u64, StatusCode> {
    Ok(time.duration_since(UNIX_EPOCH).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?.as_secs())
}
pub fn get_encryption_key() -> Result<PrivateDecryptingKey, StatusCode> {
    let key = PrivateDecryptingKey::from_pkcs8(ENCRYPTION_KEY.get().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(key)
}
pub fn get_signing_key() -> Result<Ed25519KeyPair, StatusCode> {
    let key = Ed25519KeyPair::from_pkcs8(SIGNING_KEY.get().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(key)
}
pub fn decrypt_body(body: Bytes) -> Result<Vec<u8>, StatusCode> {
    let key = get_encryption_key()?;
    let key = OaepPrivateDecryptingKey::new(key).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let mut decrypted = vec![0; key.min_output_size()];
    let decrypted = key.decrypt(&OAEP_SHA512_MGF1SHA512, &body, &mut decrypted, None).or(Err(StatusCode::BAD_REQUEST))?;
    Ok(decrypted.to_vec())
}
pub async fn validate_signature(headers: &HeaderMap, uri: Uri, body: &Bytes, method: &str) -> Result<String, StatusCode> {
    let signature = headers.get("X-Request-Signature").ok_or(StatusCode::UNAUTHORIZED)?;
    let timestamp = headers.get("X-Request-Timestamp").ok_or(StatusCode::UNAUTHORIZED)?.to_str().or(Err(StatusCode::UNAUTHORIZED))?;
    let nonce = Uuid::parse_str(headers.get("X-Request-Nonce").ok_or(StatusCode::UNAUTHORIZED)?.to_str().or(Err(StatusCode::UNAUTHORIZED))?).or(Err(StatusCode::UNAUTHORIZED))?;
    let timestamp: u64 = timestamp.parse().or(Err(StatusCode::UNAUTHORIZED))?;
    let signature = BASE64_STANDARD.decode(signature).or(Err(StatusCode::UNAUTHORIZED))?;
    let pubkey = authenticate(headers)?.1.ok_or(StatusCode::UNAUTHORIZED)?;
    let uri_hashed = BASE64_STANDARD.encode(Sha512::digest(uri.to_string()));
    let body_hashed = BASE64_STANDARD.encode(Sha512::digest(body));
    let hash = format!("{}-{}-{}-{}-{}", method, timestamp, nonce, uri_hashed, body_hashed);
    pubkey.verify_sig(hash.as_bytes(), &signature).or(Err(StatusCode::UNAUTHORIZED))?;
    validate_timestamp(timestamp)?;
    validate_nonce(nonce).await?;
    Ok(BASE64_STANDARD.encode(signature))
}
pub fn validate_timestamp(timestamp: u64) -> Result<(), StatusCode> {
    let now = SystemTime::now();
    let now = now.duration_since(UNIX_EPOCH).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?.as_secs();
    if now + 30 < timestamp || now - 30 > timestamp {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(())
}
pub fn sign_response<T: Serialize>(request_signature: String, json: Option<T>) -> Result<HeaderMap, StatusCode> {
    let mut headers = HeaderMap::new();
    let body = match json {
        Some(data) => serde_json::to_string(&data).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?,
        None => String::new(),
    };
    let body_hashed = BASE64_STANDARD.encode(Sha512::digest(body));
    let hash = format!("{}-{}", request_signature, body_hashed);
    let signing_key = get_signing_key()?;
    let signature = BASE64_STANDARD.encode(signing_key.sign(hash.as_bytes()));
    headers.insert("X-Response-Signature", HeaderValue::from_str(&signature).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?);
    Ok(headers)
}
