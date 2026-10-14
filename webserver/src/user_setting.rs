use crate::{
    auth::authenticate,
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
use diesel_async::RunQueryDsl;
use serde::Deserialize;
use shared_server::{get_user_setting, models::UserSetting, notify_server, schema::user_settings};
use std::collections::HashMap;

#[derive(Deserialize)]
pub struct UpdateUserSettingRequest {
    value: String,
}

pub async fn get_user_settings(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<(HeaderMap, Json<HashMap<String, String>>), StatusCode> {
    let user = authenticate(&headers).await?.0;
    let signature = validate_signature(&headers, uri.0, &body, "GET").await?;
    let mut db = get_db().await?;
    let allowed_settings = [
        "development_environment_settings",
        "development_environment_extensions",
        "development_environment_gitconfig",
        "development_environment_paths",
    ];
    let mut settings = HashMap::new();
    for name in allowed_settings {
        let setting = get_user_setting(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?, name.to_string(), &mut db)
            .await
            .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
        settings.insert(setting.name, setting.value);
    }
    Ok((sign_response(signature, Some(&settings))?, Json(settings)))
}
pub async fn update_user_setting(headers: HeaderMap, Path(name): Path<String>, uri: OriginalUri, body: Bytes) -> Result<HeaderMap, StatusCode> {
    let user = authenticate(&headers).await?.0;
    let uid = user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let signature = validate_signature(&headers, uri.0, &body, "PUT").await?;
    let request: UpdateUserSettingRequest = serde_json::from_slice(&decrypt_body(&headers, body)?).or(Err(StatusCode::BAD_REQUEST))?;
    let mut db = get_db().await?;
    let allowed_settings = [
        "development_environment_settings",
        "development_environment_extensions",
        "development_environment_gitconfig",
        "development_environment_paths",
    ];
    if !allowed_settings.contains(&name.as_str()) {
        return Err(StatusCode::NOT_FOUND);
    }
    if user_settings::table
        .filter(user_settings::uid.eq(uid).and(user_settings::name.eq(&name)))
        .get_result::<UserSetting>(&mut db)
        .await
        .optional()
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?
        .is_some()
    {
        diesel::update(user_settings::table.filter(user_settings::uid.eq(uid).and(user_settings::name.eq(&name))))
            .set(user_settings::value.eq(request.value))
            .execute(&mut db)
            .await
            .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    } else {
        let setting = UserSetting { uid, name, value: request.value };
        diesel::insert_into(user_settings::table)
            .values(setting)
            .execute(&mut db)
            .await
            .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    }
    notify_server(&format!("WEB:REFRESH:{}", uid)).await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    sign_response::<String>(signature, None)
}
