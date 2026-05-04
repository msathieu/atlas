use crate::get_db;
use aws_lc_rs::rsa::{OAEP_SHA512_MGF1SHA512, OaepPrivateDecryptingKey, PrivateDecryptingKey};
use axum::{body::Bytes, http::StatusCode};
use base64::{Engine, prelude::BASE64_STANDARD};
use diesel::prelude::*;
use shared_server::{models::Setting, schema};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn convert_timestamp(time: SystemTime) -> Result<u64, StatusCode> {
    Ok(time.duration_since(UNIX_EPOCH).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?.as_secs())
}
pub fn get_encryption_key() -> Result<PrivateDecryptingKey, StatusCode> {
    let mut db = get_db()?;
    let encoded = schema::settings::table.find("web_encryption_key").first::<Setting>(&mut db).unwrap().value;
    let key = PrivateDecryptingKey::from_pkcs8(&BASE64_STANDARD.decode(encoded).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(key)
}
pub fn decrypt_body(body: Bytes) -> Result<String, StatusCode> {
    let key = get_encryption_key()?;
    let key = OaepPrivateDecryptingKey::new(key).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let mut decrypted = vec![0; key.min_output_size()];
    let decrypted = key.decrypt(&OAEP_SHA512_MGF1SHA512, &body, &mut decrypted, None).or(Err(StatusCode::BAD_REQUEST))?;
    let decrypted = String::from_utf8(decrypted.to_owned()).or(Err(StatusCode::BAD_REQUEST))?;
    Ok(decrypted)
}
