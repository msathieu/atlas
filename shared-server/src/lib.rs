#![forbid(unsafe_code)]

use diesel::{
    PgConnection,
    r2d2::{ConnectionManager, Pool},
};
use fred::{
    clients::SubscriberClient,
    prelude::{Client, ClientLike, Config, PubsubInterface},
    types::Builder,
};
use std::{env, error::Error};

pub mod binary;
pub mod models;
pub mod schema;

pub fn get_db_pool() -> Result<Pool<ConnectionManager<PgConnection>>, Box<dyn Error>> {
    let url = format!(
        "postgres://{}:{}@{}/{}",
        env::var("DATABASE_USER").unwrap_or("app".to_string()),
        env::var("DATABASE_PASSWORD").unwrap_or("".to_string()),
        env::var("DATABASE_HOST").unwrap_or("localhost".to_string()),
        env::var("DATABASE_DB").unwrap_or("app".to_string())
    );
    let connection_manager = ConnectionManager::new(url);
    let pool = Pool::builder().build(connection_manager)?;
    Ok(pool)
}
fn get_pubsub_config() -> Result<Config, fred::error::Error> {
    let url = format!("valkey://{}", env::var("VALKEY_HOST").unwrap_or("localhost".to_string()));
    let mut config = Config::from_url(&url)?;
    config.version = fred::types::RespVersion::RESP3;
    Ok(config)
}
pub async fn get_sub_client() -> Result<SubscriberClient, Box<dyn Error>> {
    let client = Builder::from_config(get_pubsub_config()?).build_subscriber_client()?;
    client.init().await?;
    client.manage_subscriptions();
    Ok(client)
}
pub async fn get_pub_client() -> Result<Client, Box<dyn Error>> {
    let client = Builder::from_config(get_pubsub_config()?).build()?;
    client.init().await?;
    Ok(client)
}
pub async fn notify_server(component: &str) -> Result<(), Box<dyn Error>> {
    let pub_client = get_pub_client().await?;
    let _: () = pub_client.publish(component, "").await?;
    pub_client.quit().await?;
    Ok(())
}
