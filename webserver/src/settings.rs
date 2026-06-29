use std::collections::HashMap;

use crate::{
    auth::require_admin,
    get_db,
    util::{decrypt_body, validate_signature},
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

pub async fn get_settings(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<Json<HashMap<String, String>>, StatusCode> {
    require_admin(&headers)?;
    validate_signature(&headers, uri.0, &body, "GET")?;
    let mut db = get_db()?;
    let allowed_settings = [
        "allow_new_devices",
        "apiserver_host",
        "mail_server",
        "mail_username",
        "mail_from",
        "mail_to",
        "llm_default_provider",
        "llm_generic_endpoint",
        "llm_generic_model",
        "llm_openrouter_model",
        "tls_cert",
    ];
    let mut settings = HashMap::new();
    for name in allowed_settings {
        let setting: Setting = settings::table.find(name).get_result(&mut db).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
        settings.insert(setting.name, setting.value);
    }
    Ok(Json(settings))
}
pub async fn update_setting(headers: HeaderMap, Path(name): Path<String>, uri: OriginalUri, body: Bytes) -> Result<(), StatusCode> {
    require_admin(&headers)?;
    validate_signature(&headers, uri.0, &body, "PUT")?;
    let request: UpdateSettingRequest = serde_json::from_str(&decrypt_body(body)?).or(Err(StatusCode::BAD_REQUEST))?;
    let mut db = get_db()?;
    let allowed_settings = [
        "allow_new_devices",
        "apiserver_host",
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
    ];
    if !allowed_settings.contains(&name.as_str()) {
        return Err(StatusCode::NOT_FOUND);
    }
    diesel::update(settings::table.find(&name))
        .set(settings::value.eq(request.value))
        .execute(&mut db)
        .or(Err(StatusCode::BAD_REQUEST))?;
    notify_server("WEB:REFRESH").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(())
}
pub async fn restart_system(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<(), StatusCode> {
    require_admin(&headers)?;
    validate_signature(&headers, uri.0, &body, "POST")?;
    notify_server("MASTER:RESTART").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server("API:RESTART").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server("WEB:RESTART").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(())
}
