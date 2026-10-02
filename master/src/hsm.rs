use diesel_async::{AsyncPgConnection, pooled_connection::deadpool::Pool};
use redis::AsyncTypedCommands;
use shared_server::{get_valkey_client, hsm::HSMSigningValkey, wait_for_pubsub_msg};
use std::error::Error;

pub async fn process_hsm_requests(_pool: Pool<AsyncPgConnection>) -> Result<(), Box<dyn Error>> {
    let (mut valkey_client, mut receiver) = get_valkey_client().await?;
    valkey_client.subscribe("MASTER:HSM:SIGN").await?;
    loop {
        let uuid = wait_for_pubsub_msg(&mut receiver).await;
        let data: String = valkey_client.get_del(format!("HSM_SIGNING_REQUEST:{}", uuid)).await?.ok_or("Missing signing request")?;
        let _data: HSMSigningValkey = serde_json::from_str(&data)?;
    }
}
