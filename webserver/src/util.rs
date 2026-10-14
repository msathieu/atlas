use crate::{SIGNING_KEY, auth::authenticate, get_db};
use aws_lc_rs::{
    aead::{AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey},
    rsa::{OAEP_SHA512_MGF1SHA512, OaepPrivateDecryptingKey, PrivateDecryptingKey},
    signature::Ed25519KeyPair,
};
use axum::{
    body::Bytes,
    http::{HeaderMap, HeaderValue, StatusCode, Uri},
};
use base64::{Engine, prelude::BASE64_STANDARD};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use redis::{AsyncTypedCommands, ExistenceCheck, SetExpiry, SetOptions};
use serde::Serialize;
use sha2::{Digest, Sha512};
use shared_server::{get_valkey_client, models::Setting, parse_pubkey, schema::settings};
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
        .set_options(key, true, SetOptions::default().conditional_set(ExistenceCheck::NX).with_expiration(SetExpiry::EX(600)))
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
pub async fn get_encryption_key() -> Result<PrivateDecryptingKey, StatusCode> {
    let mut db = get_db().await?;
    let key: Setting = settings::table.find("web_encryption_key").first(&mut db).await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    parse_encryption_key(&key.value)
}
fn parse_encryption_key(key: &str) -> Result<PrivateDecryptingKey, StatusCode> {
    let key = BASE64_STANDARD.decode(key).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let key = PrivateDecryptingKey::from_pkcs8(&key).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(key)
}
pub fn get_signing_key() -> Result<Ed25519KeyPair, StatusCode> {
    let key = Ed25519KeyPair::from_pkcs8(SIGNING_KEY.get().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(key)
}
pub async fn decrypt_body(headers: &HeaderMap, body: Bytes) -> Result<Vec<u8>, StatusCode> {
    let mut db = get_db().await?;
    let keys: Vec<Setting> = settings::table
        .filter(settings::name.eq_any(["web_encryption_key", "web_encryption_key_old"]))
        .load(&mut db)
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let mut current = String::new();
    let mut old = None;
    for i in keys {
        if i.name == "web_encryption_key" {
            current = i.value.clone();
        } else if i.name == "web_encryption_key_old" && !i.value.is_empty() {
            old = Some(i.value);
        }
    }
    decrypt_body_with_keys(headers, body, &current, old)
}
fn decrypt_symmetric_key(key: &str, encrypted_key: &[u8]) -> Result<Vec<u8>, StatusCode> {
    let key = OaepPrivateDecryptingKey::new(parse_encryption_key(key)?).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let mut decrypted = vec![0; key.min_output_size()];
    let decrypted = key.decrypt(&OAEP_SHA512_MGF1SHA512, encrypted_key, &mut decrypted, None).or(Err(StatusCode::BAD_REQUEST))?;
    Ok(decrypted.to_vec())
}
fn decrypt_body_with_keys(headers: &HeaderMap, body: Bytes, current: &str, old: Option<String>) -> Result<Vec<u8>, StatusCode> {
    let encrypted_key = headers.get("X-Encryption-Key").ok_or(StatusCode::BAD_REQUEST)?;
    let encrypted_key = BASE64_STANDARD.decode(encrypted_key).or(Err(StatusCode::BAD_REQUEST))?;
    let nonce = headers.get("X-Encryption-Nonce").ok_or(StatusCode::BAD_REQUEST)?;
    let nonce = BASE64_STANDARD.decode(nonce).or(Err(StatusCode::BAD_REQUEST))?;
    let nonce = Nonce::try_assume_unique_for_key(&nonce).or(Err(StatusCode::BAD_REQUEST))?;
    let decrypted = match decrypt_symmetric_key(current, &encrypted_key) {
        Ok(key) => key,
        Err(StatusCode::BAD_REQUEST) => decrypt_symmetric_key(&old.ok_or(StatusCode::BAD_REQUEST)?, &encrypted_key)?,
        Err(error) => return Err(error),
    };
    let key = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &decrypted).or(Err(StatusCode::BAD_REQUEST))?);
    let mut ciphertext = body.to_vec();
    let plaintext = key.open_in_place(nonce, Aad::empty(), &mut ciphertext).or(Err(StatusCode::BAD_REQUEST))?;
    Ok(plaintext.to_vec())
}
pub async fn validate_signature(headers: &HeaderMap, uri: Uri, body: &Bytes, method: &str) -> Result<String, StatusCode> {
    let signature = headers.get("X-Request-Signature").ok_or(StatusCode::UNAUTHORIZED)?;
    let timestamp = headers.get("X-Request-Timestamp").ok_or(StatusCode::UNAUTHORIZED)?.to_str().or(Err(StatusCode::UNAUTHORIZED))?;
    let nonce = Uuid::parse_str(headers.get("X-Request-Nonce").ok_or(StatusCode::UNAUTHORIZED)?.to_str().or(Err(StatusCode::UNAUTHORIZED))?).or(Err(StatusCode::UNAUTHORIZED))?;
    let timestamp: u64 = timestamp.parse().or(Err(StatusCode::UNAUTHORIZED))?;
    let signature = BASE64_STANDARD.decode(signature).or(Err(StatusCode::UNAUTHORIZED))?;
    let pubkey = authenticate(headers).await?.1.ok_or(StatusCode::UNAUTHORIZED)?;
    let pubkey = parse_pubkey(pubkey).or(Err(StatusCode::UNAUTHORIZED))?;
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
    if now + 60 < timestamp || now - 60 > timestamp {
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
