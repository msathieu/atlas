use crate::tpm;
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use diesel_async::{AsyncPgConnection, pooled_connection::deadpool::Pool};
use redis::AsyncTypedCommands;
use shared_api::CryptoBackend;
use shared_server::hsm::{HSMOperation, SSHKey};
use shared_server::{get_valkey_client, notify_server_with_msg, wait_for_pubsub_msg};
use signature::Signer;
use ssh_key::Algorithm::Ed25519;
use ssh_key::{LineEnding, PrivateKey};
use std::error::Error;
use tokio::task::spawn_blocking;

pub async fn process_hsm_requests(_pool: Pool<AsyncPgConnection>) -> Result<(), Box<dyn Error>> {
    let (mut valkey_client, mut receiver) = get_valkey_client().await?;
    valkey_client.subscribe("MASTER:HSM:REQUEST").await?;
    loop {
        let uuid = wait_for_pubsub_msg(&mut receiver).await?;
        let data: String = valkey_client.get_del(format!("HSM_REQUEST:{uuid}")).await?.ok_or("Missing HSM request")?;
        let data: HSMOperation = serde_json::from_str(&data)?;
        let response = match data {
            HSMOperation::GenerateSSHKey(backend) => serde_json::to_string(&match backend {
                CryptoBackend::Software => {
                    let ssh_key = PrivateKey::random(&mut rand::rng(), Ed25519)?;
                    let ssh_key_exported = ssh_key.to_openssh(LineEnding::LF)?;
                    SSHKey::Software(ssh_key_exported.to_string())
                }
                CryptoBackend::TPM => {
                    let (private, public) = spawn_blocking(|| tpm::generate_ssh_key().map_err(|e| e.to_string())).await??;
                    SSHKey::TPM(private, public)
                }
            })?,
            HSMOperation::SignSSHRequest(key, data) => BASE64_STANDARD.encode({
                let key: SSHKey = serde_json::from_str(&key)?;
                match key {
                    SSHKey::Software(key) => {
                        let key = PrivateKey::from_openssh(key)?;
                        key.try_sign(&data)?.as_bytes().to_vec()
                    }
                    SSHKey::TPM(key, _) => spawn_blocking(move || tpm::sign_ssh(&key, &data).map_err(|e| e.to_string())).await??,
                }
            }),
        };
        notify_server_with_msg(&format!("HSM_RESPONSE:{uuid}"), &response).await?;
    }
}
