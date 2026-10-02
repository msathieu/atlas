use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl, pooled_connection::deadpool::Pool};
use redis::AsyncTypedCommands;
use shared_server::hsm::{HSMKey, HSMSigningValkey};
use shared_server::models::DevelopmentEnvironment;
use shared_server::schema::development_environments;
use shared_server::{get_valkey_client, notify_server_with_msg, wait_for_pubsub_msg};
use signature::Signer;
use ssh_key::PrivateKey;
use std::error::Error;

pub async fn process_hsm_requests(pool: Pool<AsyncPgConnection>) -> Result<(), Box<dyn Error>> {
    let (mut valkey_client, mut receiver) = get_valkey_client().await?;
    valkey_client.subscribe("MASTER:HSM:SIGN").await?;
    loop {
        let uuid = wait_for_pubsub_msg(&mut receiver).await;
        let data: String = valkey_client.get_del(format!("HSM_SIGNING_REQUEST:{}", uuid)).await?.ok_or("Missing signing request")?;
        let data: HSMSigningValkey = serde_json::from_str(&data)?;
        let signature = match data.key {
            HSMKey::SSH(environment_uuid) => {
                let mut db = pool.get().await?;
                let development_environment: DevelopmentEnvironment = development_environments::table.find(environment_uuid).first(&mut db).await?;
                drop(db);
                let key = PrivateKey::from_openssh(development_environment.ssh_key)?;
                key.try_sign(&data.data)?.as_bytes().to_vec()
            }
        };
        notify_server_with_msg(&format!("HSM_SIGNATURE:{}", uuid), &BASE64_STANDARD.encode(signature)).await?;
    }
}
