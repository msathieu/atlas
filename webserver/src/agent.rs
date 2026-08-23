use crate::{
    auth::{authenticate, generate_jwt},
    get_db,
    util::{sign_response, validate_signature},
};
use axum::{
    Json,
    body::Bytes,
    extract::{OriginalUri, Query},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
use shared_api::{SUPPORTED_ARCH, SUPPORTED_OS};
use shared_server::{authenticate_jwt, binary::personalize_binary};
use std::env;

#[derive(Deserialize)]
pub struct DownloadAuth {
    pub auth: String,
    pub arch: String,
    pub os: String,
}
#[derive(Deserialize)]
pub struct GetAgentScriptRequest {
    pub arch: String,
    pub os: String,
}
#[derive(Serialize)]
pub struct DownloadAuthResponse {
    jwt: String,
}
#[derive(Serialize)]
pub struct GetAgentScriptResponse {
    script: String,
}

pub async fn get_agent_script(headers: HeaderMap, uri: OriginalUri, request: Query<GetAgentScriptRequest>, body: Bytes) -> Result<(HeaderMap, Json<GetAgentScriptResponse>), StatusCode> {
    let user = authenticate(&headers).await?.0;
    let signature = validate_signature(&headers, uri.0, &body, "GET").await?;
    let mut db = get_db().await?;
    let agent = match personalize_binary(&mut db, &request.arch, &request.os).await {
        Ok(agent) => agent,
        Err(_) => {
            let response = GetAgentScriptResponse {
                script: "No binary found".to_string(),
            };
            return Ok((sign_response(signature, Some(&response))?, Json(response)));
        }
    };
    let hash = Sha512::digest(&agent).to_vec();
    let jwt = generate_jwt("webserver-download", &user, 10 * 60, None)?;
    let web_host = env::var("WEBSERVER_HOST").or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let script = format!(
        r#"curl -fO "https://{}/api/atlas-agent?auth={}&arch={}&os={}" &&\
sha512sum -c <<< "{}  atlas-agent" &&\
chmod +x atlas-agent &&\
./atlas-agent install &&\
rm atlas-agent
"#,
        web_host,
        jwt,
        request.arch,
        request.os,
        hex::encode(&hash)
    );
    let response = GetAgentScriptResponse { script: script.clone() };
    Ok((sign_response(signature, Some(&response))?, Json(response)))
}
pub async fn download_agent(query: Query<DownloadAuth>) -> Result<Vec<u8>, StatusCode> {
    let mut db = get_db().await?;
    authenticate_jwt(&query.auth, "webserver-download", Some(&mut db), false).await.or(Err(StatusCode::UNAUTHORIZED))?;
    if !SUPPORTED_ARCH.contains(&query.arch.as_str()) {
        return Err(StatusCode::BAD_REQUEST);
    }
    if !SUPPORTED_OS.contains(&query.os.as_str()) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let binary = personalize_binary(&mut db, &query.arch, &query.os).await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(binary)
}
pub async fn download_auth(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<(HeaderMap, Json<DownloadAuthResponse>), StatusCode> {
    let user = authenticate(&headers).await?.0;
    let signature = validate_signature(&headers, uri.0, &body, "POST").await?;
    let jwt = generate_jwt("webserver-download", &user, 10 * 60, None)?;
    let response = DownloadAuthResponse { jwt };
    Ok((sign_response(signature, Some(&response))?, Json(response)))
}
