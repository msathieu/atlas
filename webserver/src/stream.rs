use crate::{
    auth::{authenticate, authenticate_jwt, generate_jwt},
    util::validate_signature,
};
use axum::{
    Json,
    body::Bytes,
    extract::{OriginalUri, Query, WebSocketUpgrade, ws::WebSocket},
    http::{HeaderMap, StatusCode},
    response::Response,
};
use serde::{Deserialize, Serialize};
use shared_server::{get_pubsub_client, wait_for_pubsub_msg};
use std::{error::Error, time::Duration};
use tokio::time::timeout;

#[derive(Deserialize)]
pub struct StreamAuth {
    pub auth: String,
}
#[derive(Serialize)]
pub struct StreamAuthResponse {
    jwt: String,
}

pub async fn stream_auth(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<Json<StreamAuthResponse>, StatusCode> {
    let user = authenticate(&headers)?.0;
    validate_signature(&headers, uri.0, &body, "POST")?;
    let jwt = generate_jwt("webserver-stream", &user, 60, None)?;
    let response = StreamAuthResponse { jwt };
    Ok(Json(response))
}
pub async fn stream(query: Query<StreamAuth>, socket: WebSocketUpgrade) -> Response {
    if authenticate_jwt(&query.auth, "webserver-stream").is_err() {
        return Response::new("Unauthorized".into());
    }
    socket.on_upgrade(stream_wrapper)
}
async fn stream_wrapper(socket: WebSocket) {
    stream_handler(socket).await.ok();
}
async fn stream_handler(mut socket: WebSocket) -> Result<(), Box<dyn Error>> {
    let (mut pubsub_client, mut receiver) = get_pubsub_client().await?;
    pubsub_client.subscribe("WEB:REFRESH").await?;
    loop {
        if timeout(Duration::from_secs(30), async { wait_for_pubsub_msg(&mut receiver).await }).await.is_ok() {
            socket.send("POLL".into()).await?;
        } else {
            socket.send("".into()).await?;
        }
        socket.recv().await.ok_or("No message received")??;
    }
}
