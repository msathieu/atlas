use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl, pooled_connection::deadpool::Pool};
use hmac::{Hmac, KeyInit, Mac};
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::Value;
use sha2::Sha256;
use shared_api::ContainerState;
use shared_server::models::Agent;
use shared_server::schema::{agents, containers};
use std::env;
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};

pub async fn process_forge_actions(pool: Pool<AsyncPgConnection>) -> Result<(), Box<dyn Error>> {
    let mut db = pool.get().await?;
    let agents: Vec<Agent> = agents::table
        .filter(agents::forge_token.is_not_null().and(agents::forge_webhook_secret.is_not_null()))
        .inner_join(containers::table.on(agents::id.eq(containers::id)))
        .filter(containers::state.eq(ContainerState::Running as i32))
        .select(Agent::as_select())
        .get_results(&mut db)
        .await?;
    for agent in agents {
        handle_forge_notifications(agent).await?;
    }
    Ok(())
}
#[allow(clippy::get_first)]
async fn handle_forge_notifications(agent: Agent) -> Result<(), Box<dyn Error>> {
    let client = Client::new();
    let mut headers = HeaderMap::new();
    let forge_token = agent.forge_token.ok_or("Missing forge token")?;
    headers.insert("Authorization", HeaderValue::from_str(&format!("Bearer {}", forge_token))?);
    let response = client.get(format!("{}/api/v1/notifications", env::var("FORGE_URL")?)).headers(headers.clone()).send().await?;
    if !response.status().is_success() {
        return Err("Error from forge API".into());
    }
    let response: Value = response.json().await?;
    let response = response.as_array().ok_or("Invalid response")?;
    if !response.is_empty() {
        let notification = response.get(0).ok_or("Invalid response")?;
        let id = notification.get("id").ok_or("Invalid response")?.as_u64().ok_or("Invalid response")?;
        let response = client.patch(format!("{}/api/v1/notifications/threads/{}", env::var("FORGE_URL")?, id)).headers(headers).send().await?;
        if !response.status().is_success() {
            return Err("Error from forge API".into());
        }
        let mut webhook_headers = HeaderMap::new();
        webhook_headers.insert("Content-Type", HeaderValue::from_str("application/json")?);
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        webhook_headers.insert("X-Webhook-Timestamp", HeaderValue::from_str(&timestamp.to_string())?);
        let secret = agent.forge_webhook_secret.ok_or("Agent is missing webhook secret")?;
        let mut hmac: Hmac<Sha256> = Hmac::new_from_slice(secret.as_bytes())?;
        hmac.update(timestamp.to_string().as_bytes());
        hmac.update(b".");
        let body = notification.to_string();
        hmac.update(body.as_bytes());
        let hmac = hmac.finalize();
        let hmac = hex::encode(hmac.into_bytes());
        webhook_headers.insert("X-Webhook-Signature-V2", HeaderValue::from_str(&hmac)?);
        let response = client
            .post(format!("http://atlas-agent-{}.atlas.svc.cluster.local:8644/webhooks/forge", agent.id))
            .body(body)
            .headers(webhook_headers)
            .send()
            .await?;
        if !response.status().is_success() {
            return Err("Error sending webhook".into());
        }
    }
    Ok(())
}
