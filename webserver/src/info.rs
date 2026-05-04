use crate::util::get_encryption_key;
use aws_lc_rs::encoding::AsDer;
use axum::{Json, http::StatusCode};
use base64::{Engine, prelude::BASE64_STANDARD};
use gethostname::gethostname;
use serde::Serialize;
use shared_api::{API_VERSION, MAGIC};

#[derive(Serialize)]
pub struct APIServerInfo {
    magic: String,
    api_version: usize,
    hostname: String,
    version: String,
    pubkey: String,
}

pub async fn server_info() -> Result<Json<APIServerInfo>, StatusCode> {
    let key = get_encryption_key()?;
    let pubkey = key.public_key().as_der().or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let info = APIServerInfo {
        magic: MAGIC.to_string(),
        api_version: API_VERSION,
        hostname: gethostname().into_string().or(Err(StatusCode::INTERNAL_SERVER_ERROR))?,
        version: env!("CARGO_PKG_VERSION").to_string(),
        pubkey: BASE64_STANDARD.encode(pubkey.as_ref()),
    };
    Ok(Json(info))
}
