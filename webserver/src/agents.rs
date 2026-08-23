use crate::{
    auth::{authenticate, generate_jwt},
    get_db,
    util::{convert_timestamp, sign_response, validate_signature},
};
use axum::{
    Json,
    body::Bytes,
    extract::{OriginalUri, Path, Query},
    http::{HeaderMap, StatusCode},
};
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use diesel::prelude::*;
use diesel_async::{AsyncConnection, RunQueryDsl};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use shared_api::{ContainerOrigin, ContainerState};
use shared_server::{
    models::{Agent, Command, Container},
    notify_server,
    schema::{agents, commands, containers},
};
use std::{env, time::SystemTime};
use uuid::Uuid;

#[derive(Serialize)]
pub struct APIAgent {
    id: Uuid,
    name: String,
    created_at: u64,
    state: String,
    pending: bool,
}
#[derive(Deserialize)]
pub struct CreateAgentRequest {
    name: String,
}
#[derive(Serialize)]
pub struct CreateAgentResponse {
    id: Uuid,
}
#[derive(Serialize)]
pub struct OpenAgentResponse {
    url: String,
    ready: bool,
}
#[derive(Deserialize)]
pub struct SetAgentStateRequest {
    state: String,
}

pub async fn agents_list(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<(HeaderMap, Json<Vec<APIAgent>>), StatusCode> {
    let user = authenticate(&headers).await?.0;
    let signature = validate_signature(&headers, uri.0, &body, "GET").await?;
    let mut db = get_db().await?;
    let agents: Vec<(Agent, Container)> = agents::table
        .inner_join(containers::table.on(agents::id.eq(containers::id)))
        .filter(containers::uid.eq(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
        .select((Agent::as_select(), Container::as_select()))
        .get_results(&mut db)
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let agents: Result<Vec<APIAgent>, StatusCode> = agents
        .iter()
        .map(|(agent, container)| {
            let state = match ContainerState::try_from(container.state as u32).or(Err(StatusCode::INTERNAL_SERVER_ERROR))? {
                ContainerState::NonExistent => "Non-Existent",
                ContainerState::Started => "Started",
                ContainerState::Running => "Running",
                ContainerState::Stopped => "Stopped",
                ContainerState::Restarting => "Restarting",
            }
            .to_string();
            Ok(APIAgent {
                id: agent.id,
                name: agent.name.clone(),
                created_at: convert_timestamp(container.created_at)?,
                state,
                pending: container.requested_state != container.state,
            })
        })
        .collect();
    let agents = agents?;
    Ok((sign_response(signature, Some(&agents))?, Json(agents)))
}
pub async fn create_agent(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<(HeaderMap, Json<CreateAgentResponse>), StatusCode> {
    let user = authenticate(&headers).await?.0;
    let signature = validate_signature(&headers, uri.0, &body, "POST").await?;
    let request: CreateAgentRequest = serde_json::from_slice(&body).or(Err(StatusCode::BAD_REQUEST))?;
    if request.name.is_empty() || !request.name.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut db = get_db().await?;
    let mut container = Container {
        id: None,
        created_at: SystemTime::now(),
        uid: user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?,
        origin: ContainerOrigin::Agent as i32,
        state: ContainerState::NonExistent as i32,
        requested_state: ContainerState::Stopped as i32,
        image: "[AGENT]".to_string(),
        storage_size: 25,
        memory_limit: 16,
    };
    let mut id = Uuid::nil();
    let password: [u8; 64] = rand::random();
    let password = BASE64_STANDARD.encode(password);
    db.transaction(async |db| {
        container = diesel::insert_into(containers::table).values(container).get_result(db).await?;
        id = container.id.ok_or(diesel::result::Error::NotFound)?;
        let agent = Agent {
            id,
            name: request.name,
            command: None,
            password,
        };
        diesel::insert_into(agents::table).values(agent).execute(db).await?;
        Ok::<(), diesel::result::Error>(())
    })
    .await
    .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server("MASTER:CONTAINERS").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server(&format!("WEB:REFRESH:{}", user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let response = CreateAgentResponse { id };
    Ok((sign_response(signature, Some(&response))?, Json(response)))
}
pub async fn set_agent_state(headers: HeaderMap, Path(id): Path<Uuid>, request: Query<SetAgentStateRequest>, uri: OriginalUri, body: Bytes) -> Result<HeaderMap, StatusCode> {
    let user = authenticate(&headers).await?.0;
    let signature = validate_signature(&headers, uri.0, &body, "POST").await?;
    let mut db = get_db().await?;
    let state = match request.state.as_str() {
        "stop" => ContainerState::Stopped,
        "start" => ContainerState::Running,
        "delete" => ContainerState::NonExistent,
        _ => return Err(StatusCode::BAD_REQUEST),
    };
    diesel::update(
        containers::table.filter(
            containers::id
                .eq(id)
                .and(containers::uid.eq(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
                .and(containers::origin.eq(ContainerOrigin::Agent as i32)),
        ),
    )
    .set(containers::requested_state.eq(state as i32))
    .execute(&mut db)
    .await
    .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server("MASTER:CONTAINERS").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server(&format!("WEB:REFRESH:{}", user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    sign_response::<String>(signature, None)
}
pub async fn open_agent(headers: HeaderMap, Path(id): Path<Uuid>, uri: OriginalUri, body: Bytes) -> Result<(HeaderMap, Json<OpenAgentResponse>), StatusCode> {
    let user = authenticate(&headers).await?.0;
    let signature = validate_signature(&headers, uri.0, &body, "POST").await?;
    let mut db = get_db().await?;
    let container: Container = containers::table
        .filter(
            containers::uid
                .eq(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?)
                .and(containers::origin.eq(ContainerOrigin::Agent as i32))
                .and(containers::id.eq(id)),
        )
        .first(&mut db)
        .await
        .or(Err(StatusCode::NOT_FOUND))?;
    let mut ready = true;
    if container.state != ContainerState::Running as i32 {
        ready = false;
    }
    let agent: Agent = agents::table.find(id).first(&mut db).await.or(Err(StatusCode::NOT_FOUND))?;
    if let Some(command) = agent.command {
        let command: Command = commands::table.find(command).first(&mut db).await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
        if !command.completed {
            ready = false;
        }
    } else {
        ready = false;
    }
    let webserver = env::var("WEBSERVER_HOST").or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let jwt = generate_jwt(&format!("proxy-agent-{}", id), &user, 12 * 60 * 60, None)?;
    let url = format!("https://{}.agent.{}/atlas_setup?jwt={}", id, webserver, jwt);
    let response = OpenAgentResponse { url, ready };
    Ok((sign_response(signature, Some(&response))?, Json(response)))
}
pub async fn deliver_webhook(headers: HeaderMap, Path((id, name)): Path<(Uuid, String)>, body: Bytes) -> Result<StatusCode, StatusCode> {
    let mut db = get_db().await?;
    let _: Container = containers::table
        .filter(
            containers::origin
                .eq(ContainerOrigin::Agent as i32)
                .and(containers::id.eq(id))
                .and(containers::state.eq(ContainerState::Running as i32)),
        )
        .first(&mut db)
        .await
        .or(Err(StatusCode::NOT_FOUND))?;
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut request_headers = HeaderMap::new();
    for i in headers {
        if let Some(name) = i.0
            && name.to_string().to_lowercase().starts_with("x-")
        {
            request_headers.append(name, i.1);
        }
    }
    let client = Client::new();
    let response = client
        .post(format!("http://atlas-agent-{}.atlas.svc.cluster.local:8644/webhooks/{}", id, name))
        .body(body)
        .headers(request_headers)
        .send()
        .await
        .or(Err(StatusCode::BAD_REQUEST))?;
    if response.status().is_success() {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}
