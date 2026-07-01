use crate::{
    auth::authenticate,
    get_db,
    util::{convert_timestamp, sign_response, validate_signature},
};
use axum::{
    Json,
    body::Bytes,
    extract::{OriginalUri, Path},
    http::{HeaderMap, StatusCode},
};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use shared_api::CommandOrigin;
use shared_server::{models::Command, notify_server, schema::commands};
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
    let user = authenticate(&headers)?.0;
    let signature = validate_signature(&headers, uri.0, &body, "GET").await?;
    let mut db = get_db()?;
    let commands: Vec<Command> = commands::table
        .filter(commands::uid.eq(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
        .get_results(&mut db)
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let commands: Result<Vec<APICommand>, StatusCode> = commands
        .iter()
        .map(|command| {
            let origin = match CommandOrigin::try_from(command.origin as u32).or(Err(StatusCode::INTERNAL_SERVER_ERROR))? {
                CommandOrigin::SystemIdentification => "System Identification",
                CommandOrigin::UserRequested => "User",
                CommandOrigin::LLMTool => "LLM Tool",
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
            })
        })
        .collect();
    let commands = commands?;
    Ok((sign_response(signature, Some(&commands))?, Json(commands)))
}
pub async fn create_command(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<(HeaderMap, Json<CreateCommandResponse>), StatusCode> {
    let user = authenticate(&headers)?.0;
    let signature = validate_signature(&headers, uri.0, &body, "POST").await?;
    let request: CreateCommandRequest = serde_json::from_str(&String::from_utf8(body.to_vec()).or(Err(StatusCode::BAD_REQUEST))?).or(Err(StatusCode::BAD_REQUEST))?;
    let mut db = get_db()?;
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
    };
    cmd = diesel::insert_into(commands::table).values(cmd).get_result(&mut db).or(Err(StatusCode::BAD_REQUEST))?;
    notify_server(&format!("API:DEVICE:{}", request.device)).await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server("WEB:REFRESH").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let response = CreateCommandResponse {
        id: cmd.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?,
    };
    Ok((sign_response(signature, Some(&response))?, Json(response)))
}
pub async fn delete_command(headers: HeaderMap, Path(id): Path<String>, uri: OriginalUri, body: Bytes) -> Result<HeaderMap, StatusCode> {
    let user = authenticate(&headers)?.0;
    let signature = validate_signature(&headers, uri.0, &body, "DELETE").await?;
    let mut db = get_db()?;
    diesel::delete(
        commands::table.filter(
            commands::id
                .eq(Uuid::parse_str(&id).or(Err(StatusCode::BAD_REQUEST))?)
                .and(commands::uid.eq(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?)),
        ),
    )
    .execute(&mut db)
    .or(Err(StatusCode::NOT_FOUND))?;
    notify_server("WEB:REFRESH").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server(&format!("MASTER:CMD:{}", id)).await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    sign_response::<String>(signature, None)
}
