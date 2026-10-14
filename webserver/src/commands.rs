use crate::{
    auth::{authenticate, require_admin},
    get_db,
    util::{convert_timestamp, decrypt_body, sign_response, validate_signature},
};
use axum::{
    Json,
    body::Bytes,
    extract::{OriginalUri, Path},
    http::{HeaderMap, StatusCode},
};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use serde::{Deserialize, Serialize};
use shared_api::{CommandLanguage, CommandOrigin};
use shared_server::{
    models::{Agent, Command, DevelopmentEnvironment},
    notify_server,
    schema::{agents, commands, development_environments},
};
use std::time::SystemTime;
use uuid::Uuid;

#[derive(Serialize)]
pub struct APICommand {
    id: Uuid,
    device: Uuid,
    code: String,
    completed: bool,
    status_code: i32,
    stdout: String,
    stderr: String,
    requested_timestamp: u64,
    completed_timestamp: Option<u64>,
    origin: String,
    language: String,
}
#[derive(Deserialize)]
pub struct CreateCommandRequest {
    device: Uuid,
    code: String,
}
#[derive(Serialize)]
pub struct CreateCommandResponse {
    id: Uuid,
}

pub async fn commands_list(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<(HeaderMap, Json<Vec<APICommand>>), StatusCode> {
    let user = authenticate(&headers).await?.0;
    let signature = validate_signature(&headers, uri.0, &body, "GET").await?;
    let mut db = get_db().await?;
    let commands: Vec<Command> = commands::table
        .filter(commands::uid.eq(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
        .get_results(&mut db)
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let commands: Result<Vec<APICommand>, StatusCode> = commands
        .iter()
        .map(|command| {
            let origin = match CommandOrigin::try_from(command.origin as u32).or(Err(StatusCode::INTERNAL_SERVER_ERROR))? {
                CommandOrigin::SystemIdentification => "System Identification",
                CommandOrigin::UserRequested => "User",
                CommandOrigin::LLMTool => "LLM Tool",
                CommandOrigin::SystemInitiated => "System Initiated",
            };
            let language = match CommandLanguage::try_from(command.language as u32).or(Err(StatusCode::INTERNAL_SERVER_ERROR))? {
                CommandLanguage::Lua => "Lua",
                CommandLanguage::Python => "Python",
            };
            Ok(APICommand {
                id: command.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?,
                device: command.device,
                code: command.code.clone(),
                completed: command.completed,
                status_code: command.status_code,
                stdout: command.stdout.clone(),
                stderr: command.stderr.clone(),
                requested_timestamp: convert_timestamp(command.requested_timestamp)?,
                completed_timestamp: command.completed_timestamp.map(|timestamp| convert_timestamp(timestamp).unwrap_or(0)),
                origin: origin.to_string(),
                language: language.to_string(),
            })
        })
        .collect();
    let commands = commands?;
    Ok((sign_response(signature, Some(&commands))?, Json(commands)))
}
pub async fn create_command(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<(HeaderMap, Json<CreateCommandResponse>), StatusCode> {
    let user = require_admin(&headers).await?;
    let signature = validate_signature(&headers, uri.0, &body, "POST").await?;
    let request: CreateCommandRequest = serde_json::from_slice(&decrypt_body(&headers, body).await?).or(Err(StatusCode::BAD_REQUEST))?;
    let mut db = get_db().await?;
    let mut cmd = Command {
        id: None,
        device: request.device,
        code: request.code,
        completed: false,
        status_code: 0,
        stdout: "".to_owned(),
        stderr: "".to_owned(),
        requested_timestamp: SystemTime::now(),
        completed_timestamp: None,
        origin: CommandOrigin::UserRequested as i32,
        device_info_type: None,
        uid: Some(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?),
        initiated: false,
        chat_id: None,
        chat_sequence: None,
        chat_tool_call_id: None,
        language: CommandLanguage::Lua as i32,
        transient: false,
    };
    cmd = diesel::insert_into(commands::table).values(cmd).get_result(&mut db).await.or(Err(StatusCode::BAD_REQUEST))?;
    notify_server(&format!("API:DEVICE:{}", request.device)).await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server(&format!("WEB:REFRESH:{}", user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let response = CreateCommandResponse {
        id: cmd.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?,
    };
    Ok((sign_response(signature, Some(&response))?, Json(response)))
}
pub async fn delete_command(headers: HeaderMap, Path(id): Path<Uuid>, uri: OriginalUri, body: Bytes) -> Result<HeaderMap, StatusCode> {
    let user = authenticate(&headers).await?.0;
    let signature = validate_signature(&headers, uri.0, &body, "DELETE").await?;
    let mut db = get_db().await?;
    if development_environments::table
        .filter(development_environments::command.eq(id))
        .first::<DevelopmentEnvironment>(&mut db)
        .await
        .optional()
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?
        .is_some()
        || agents::table
            .filter(agents::command.eq(id))
            .first::<Agent>(&mut db)
            .await
            .optional()
            .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?
            .is_some()
    {
        return Err(StatusCode::NOT_FOUND);
    }
    diesel::delete(commands::table.filter(commands::id.eq(id).and(commands::uid.eq(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))))
        .execute(&mut db)
        .await
        .or(Err(StatusCode::NOT_FOUND))?;
    notify_server(&format!("WEB:REFRESH:{}", user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server(&format!("MASTER:CMD:{}", id)).await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    sign_response::<String>(signature, None)
}
