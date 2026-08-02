use crate::{
    auth::{authenticate, generate_jwt},
    get_db,
    util::{sign_response, validate_signature},
};
use axum::{
    Json,
    body::Bytes,
    extract::{OriginalUri, Query, WebSocketUpgrade, ws::WebSocket},
    http::{HeaderMap, StatusCode},
    response::Response,
};
use serde::{Deserialize, Serialize};
use shared_server::{authenticate_jwt, get_valkey_client, wait_for_pubsub_msg};
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

pub async fn stream_auth(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<(HeaderMap, Json<StreamAuthResponse>), StatusCode> {
    let user = authenticate(&headers)?.0;
    let signature = validate_signature(&headers, uri.0, &body, "POST").await?;
    let jwt = generate_jwt("webserver-stream", &user, 60, None)?;
    let response = StreamAuthResponse { jwt };
    Ok((sign_response(signature, Some(&response))?, Json(response)))
}
pub async fn stream(query: Query<StreamAuth>, socket: WebSocketUpgrade) -> Response {
    let mut db = match get_db() {
        Ok(db) => db,
        Err(_) => return Response::new("Internal Server Error".into()),
    };
    if authenticate_jwt(&query.auth, "webserver-stream", Some(&mut db), false).is_err() {
        return Response::new("Unauthorized".into());
    }
    socket.on_upgrade(stream_wrapper)
}
async fn stream_wrapper(socket: WebSocket) {
    stream_handler(socket).await.ok();
}
async fn stream_handler(mut socket: WebSocket) -> Result<(), Box<dyn Error>> {
    let (mut valkey_client, mut receiver) = get_valkey_client().await?;
    valkey_client.subscribe("WEB:REFRESH").await?;
    loop {
        if let Ok(data) = timeout(Duration::from_secs(30), async { wait_for_pubsub_msg(&mut receiver).await }).await {
            if data.starts_with("SSH_FORWARD:") {
                socket.send(data.into()).await?;
            } else {
                socket.send("POLL".into()).await?;
            }
        } else {
            socket.send("".into()).await?;
        }
        socket.recv().await.ok_or("No message received")??;
    }
}
