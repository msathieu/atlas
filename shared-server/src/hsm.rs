use crate::{get_valkey_client, notify_server_with_msg, wait_for_pubsub_msg};
use redis::AsyncTypedCommands;
use serde::{Deserialize, Serialize};
use shared_api::CryptoBackend;
use ssh_key::PrivateKey;
use std::{error::Error, time::Duration};
use tokio::time::timeout;
use uuid::Uuid;

#[derive(Deserialize, Serialize)]
pub enum SSHKey {
    Software(String),
}
#[derive(Deserialize, Serialize)]
pub enum HSMOperation {
    GenerateSSHKey(CryptoBackend),
    SignSSHRequest(String, Vec<u8>),
}

pub fn get_ssh_pubkey(key: &str, comment: Option<&str>) -> Result<String, Box<dyn Error>> {
    let key: SSHKey = serde_json::from_str(key)?;
    Ok(match key {
        SSHKey::Software(key) => {
            let mut private_key = PrivateKey::from_openssh(key)?;
            if let Some(comment) = comment {
                private_key.set_comment(comment);
            }
            private_key.public_key().to_openssh()?
        }
    })
}
pub async fn request_hsm_operation(operation: HSMOperation) -> Result<String, Box<dyn Error>> {
    let (mut valkey_client, mut receiver) = get_valkey_client().await?;
    let signing_id = Uuid::new_v4();
    let valkey_key = format!("HSM_REQUEST:{}", signing_id);
    let data = serde_json::to_string(&operation)?;
    valkey_client.set_ex(valkey_key, data, 15).await.or(Err("Failed to set valkey key"))?;
    valkey_client.subscribe(format!("HSM_RESPONSE:{}", signing_id)).await?;
    notify_server_with_msg("MASTER:HSM:REQUEST", &signing_id.to_string()).await?;
    let response = timeout(Duration::from_secs(15), async { wait_for_pubsub_msg(&mut receiver).await }).await??;
    Ok(response)
}
