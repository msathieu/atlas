use crate::auth::{authenticate, authenticate_jwt, generate_jwt};
use axum::{
    Json,
    extract::{Query, WebSocketUpgrade, ws::WebSocket},
    http::{HeaderMap, StatusCode},
    response::Response,
};
use fred::{
    clients::SubscriberClient,
    prelude::{ClientLike, EventInterface, PubsubInterface},
};
use serde::{Deserialize, Serialize};
use shared_server::get_sub_client;
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

pub async fn stream_auth(headers: HeaderMap) -> Result<Json<StreamAuthResponse>, StatusCode> {
    let user = authenticate(headers).await?;
    let jwt = generate_jwt("webserver-stream", &user, 60, None).await?;
    let response = StreamAuthResponse { jwt };
    Ok(Json(response))
}
pub async fn stream(query: Query<StreamAuth>, socket: WebSocketUpgrade) -> Response {
    if authenticate_jwt(&query.auth, "webserver-stream").await.is_err() {
        return Response::new("Unauthorized".into());
    }
    socket.on_upgrade(stream_wrapper)
}
async fn stream_wrapper(socket: WebSocket) {
    let sub_client = match get_sub_client().await {
        Ok(client) => client,
        Err(e) => {
            println!("Error connecting to Valkey: {}", e);
            return;
        }
    };
    stream_handler(socket, &sub_client).await.ok();
    if let Err(e) = sub_client.quit().await {
        println!("Error closing Valkey connection: {}", e);
    }
}
async fn stream_handler(mut socket: WebSocket, sub_client: &SubscriberClient) -> Result<(), Box<dyn Error>> {
    sub_client.subscribe("WEB:REFRESH").await?;
    let mut receiver = sub_client.message_rx();
    loop {
        if timeout(Duration::from_secs(30), async { receiver.recv().await.ok() }).await.is_ok() {
            socket.send("POLL".into()).await?;
        } else {
            socket.send("".into()).await?;
        }
    }
}
