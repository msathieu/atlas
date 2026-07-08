use crate::{
    get_db,
    util::{get_encryption_key, get_signing_key, sign_response},
};
use aws_lc_rs::{encoding::AsDer, signature::KeyPair};
use axum::{
    Json,
    http::{HeaderMap, StatusCode},
};
use base64::{Engine, prelude::BASE64_STANDARD};
use diesel::prelude::*;
use gethostname::gethostname;
use serde::Serialize;
use serde_json::json;
use shared_api::{API_VERSION, MAGIC};
use shared_server::{models::Setting, schema::settings};

#[derive(Serialize)]
pub struct APIServerInfo {
    magic: String,
    api_version: usize,
    hostname: String,
    version: String,
    encryption_pubkey: String,
    signing_pubkey: String,
    environment: String,
}

pub async fn server_info() -> Result<(HeaderMap, Json<APIServerInfo>), StatusCode> {
    let encryption_key = get_encryption_key()?;
    let encryption_pubkey = encryption_key.public_key().as_der().or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let signing_key = get_signing_key()?;
    let signing_pubkey = signing_key.public_key().as_der().or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    #[cfg(not(debug_assertions))]
    let environment = "production";
    #[cfg(debug_assertions)]
    let environment = "development";
    let info = APIServerInfo {
        magic: MAGIC.to_string(),
        api_version: API_VERSION,
        hostname: gethostname().into_string().or(Err(StatusCode::INTERNAL_SERVER_ERROR))?,
        version: env!("CARGO_PKG_VERSION").to_string(),
        encryption_pubkey: BASE64_STANDARD.encode(encryption_pubkey.as_ref()),
        signing_pubkey: BASE64_STANDARD.encode(signing_pubkey.as_ref()),
        environment: environment.to_string(),
    };
    Ok((sign_response("info".to_string(), Some(&info))?, Json(info)))
}
pub async fn assetlinks() -> Result<Json<serde_json::Value>, StatusCode> {
    let mut db = get_db()?;
    let apk_hash: Setting = settings::table.find("apk_hash").get_result(&mut db).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let mut fingerprints = vec![];
    if !apk_hash.value.is_empty() {
        fingerprints.push(apk_hash.value);
    }
    let assetlinks = json!([{"relation": ["delegate_permission/common.handle_all_urls", "delegate_permission/common.get_login_creds"], "target": {"namespace": "android_app", "package_name": "com.strypsteen.atlas", "sha256_cert_fingerprints": fingerprints}}]);
    Ok(Json(assetlinks))
}
