use std::collections::HashMap;

use crate::{
    auth::require_admin,
    get_db,
    util::{decrypt_body, sign_response, validate_signature},
};
use axum::{
    Json,
    body::Bytes,
    extract::{OriginalUri, Path},
    http::{HeaderMap, StatusCode},
};
use diesel::prelude::*;
use serde::Deserialize;
use shared_server::{models::Setting, notify_server, schema::settings};

#[derive(Deserialize)]
pub struct UpdateSettingRequest {
    value: String,
}

pub async fn get_settings(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<(HeaderMap, Json<HashMap<String, String>>), StatusCode> {
    require_admin(&headers)?;
    let signature = validate_signature(&headers, uri.0, &body, "GET").await?;
    let mut db = get_db()?;
    let allowed_settings = [
        "allow_new_devices",
        "apiserver_host",
        "webserver_host",
        "mail_server",
        "mail_username",
        "mail_from",
        "mail_to",
        "llm_default_provider",
        "llm_generic_endpoint",
        "llm_generic_model",
        "llm_openrouter_model",
        "tls_cert",
        "apk_hash",
        "k8s_storageclass",
        "development_environment_image",
        "code_server_settings",
    ];
    let mut settings = HashMap::new();
    for name in allowed_settings {
        let setting: Setting = settings::table.find(name).get_result(&mut db).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
        settings.insert(setting.name, setting.value);
    }
    Ok((sign_response(signature, Some(&settings))?, Json(settings)))
}
pub async fn update_setting(headers: HeaderMap, Path(name): Path<String>, uri: OriginalUri, body: Bytes) -> Result<HeaderMap, StatusCode> {
    require_admin(&headers)?;
    let signature = validate_signature(&headers, uri.0, &body, "PUT").await?;
    let body = if name == "code_server_settings" { body.to_vec() } else { decrypt_body(body)? };
    let request: UpdateSettingRequest = serde_json::from_slice(&body).or(Err(StatusCode::BAD_REQUEST))?;
    let mut db = get_db()?;
    let allowed_settings = [
        "allow_new_devices",
        "apiserver_host",
        "webserver_host",
        "mail_server",
        "mail_username",
        "mail_password",
        "mail_from",
        "mail_to",
        "llm_default_provider",
        "llm_generic_endpoint",
        "llm_generic_key",
        "llm_generic_model",
        "llm_openrouter_key",
        "llm_openrouter_model",
        "apk_hash",
        "k8s_storageclass",
        "development_environment_image",
        "code_server_settings",
    ];
    if !allowed_settings.contains(&name.as_str()) {
        return Err(StatusCode::NOT_FOUND);
    }
    diesel::update(settings::table.find(&name))
        .set(settings::value.eq(request.value))
        .execute(&mut db)
        .or(Err(StatusCode::BAD_REQUEST))?;
    notify_server("WEB:REFRESH").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    sign_response::<String>(signature, None)
}
pub async fn restart_system(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<HeaderMap, StatusCode> {
    require_admin(&headers)?;
    let signature = validate_signature(&headers, uri.0, &body, "POST").await?;
    notify_server("MASTER:RESTART").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server("API:RESTART").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server("WEB:RESTART").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server("DEVPROXY:RESTART").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    sign_response::<String>(signature, None)
}
