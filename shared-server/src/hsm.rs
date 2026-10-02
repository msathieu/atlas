use crate::{get_valkey_client, notify_server_with_msg, wait_for_pubsub_msg};
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use redis::AsyncTypedCommands;
use serde::{Deserialize, Serialize};
use std::{error::Error, time::Duration};
use tokio::time::timeout;
use uuid::Uuid;

#[derive(Deserialize, Serialize)]
pub enum HSMKey {}

#[derive(Deserialize, Serialize)]
pub struct HSMSigningValkey {
    pub data: Vec<u8>,
    pub key: HSMKey,
}

pub async fn request_hsm_signature(key: HSMKey, data: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let (mut valkey_client, mut receiver) = get_valkey_client().await?;
    let signing_id = Uuid::new_v4();
    let valkey_key = format!("HSM_SIGNING_REQUEST:{}", signing_id);
    let data = HSMSigningValkey { data: data.to_vec(), key };
    let data = serde_json::to_string(&data)?;
    valkey_client.set_ex(valkey_key, data, 15).await.or(Err("Failed to set valkey key"))?;
    valkey_client.subscribe(format!("HSM_SIGNATURE:{}", signing_id)).await?;
    notify_server_with_msg("MASTER:HSM:SIGN", &signing_id.to_string()).await?;
    let response = timeout(Duration::from_secs(15), async { wait_for_pubsub_msg(&mut receiver).await }).await?;
    let response = BASE64_STANDARD.decode(response)?;
    Ok(response)
}
