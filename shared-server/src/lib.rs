#![forbid(unsafe_code)]

use argon2::{
    Algorithm, Argon2, Params, PasswordHasher, Version,
    password_hash::{SaltString, rand_core::OsRng},
};
use diesel::{
    PgConnection,
    r2d2::{ConnectionManager, Pool},
};
use redis::{AsyncCommands, PushInfo, PushKind, RedisError, aio::ConnectionManagerConfig};
use std::{env, error::Error, process::exit, sync::OnceLock};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

pub mod binary;
pub mod models;
pub mod schema;

static PUBSUB_POOL: OnceLock<redis::aio::ConnectionManager> = OnceLock::new();

pub fn get_db_pool() -> Result<Pool<ConnectionManager<PgConnection>>, Box<dyn Error>> {
    let url = format!(
        "postgres://{}:{}@{}/{}?sslmode=require",
        env::var("DATABASE_USER").unwrap_or("app".to_string()),
        env::var("DATABASE_PASSWORD").unwrap_or("".to_string()),
        env::var("DATABASE_HOST").unwrap_or("localhost".to_string()),
        env::var("DATABASE_DB").unwrap_or("app".to_string())
    );
    let connection_manager = ConnectionManager::new(url);
    let pool = Pool::builder().build(connection_manager)?;
    Ok(pool)
}
pub async fn get_valkey_client() -> Result<(redis::aio::ConnectionManager, UnboundedReceiver<PushInfo>), RedisError> {
    let url = format!(
        "redis://:{}@{}/?protocol=resp3",
        env::var("VALKEY_PASSWORD").unwrap_or("".to_string()),
        env::var("VALKEY_HOST").unwrap_or("localhost".to_string())
    );
    let client = redis::Client::open(url)?;
    let mut config = ConnectionManagerConfig::new();
    config = config.set_automatic_resubscription();
    let (tx, rx) = unbounded_channel();
    config = config.set_push_sender(tx);
    let manager = redis::aio::ConnectionManager::new_with_config(client, config).await?;
    Ok((manager, rx))
}
pub async fn notify_server(channel: &str) -> Result<(), Box<dyn Error>> {
    if PUBSUB_POOL.get().is_none() {
        let client = get_valkey_client().await?;
        PUBSUB_POOL.set(client.0).ok();
    }
    PUBSUB_POOL.get().ok_or("Failed to get pubsub client")?.clone().publish::<&str, &str, ()>(channel, "").await?;
    Ok(())
}
pub async fn wait_for_pubsub_msg(receiver: &mut UnboundedReceiver<PushInfo>) {
    loop {
        if let Some(msg) = receiver.recv().await
            && msg.kind == PushKind::Message
        {
            return;
        }
    }
}
async fn listen_for_restarts_task(component: String) -> Result<(), RedisError> {
    let (mut manager, mut receiver) = get_valkey_client().await?;
    manager.subscribe(format!("{}:RESTART", component)).await?;
    wait_for_pubsub_msg(&mut receiver).await;
    Ok(())
}
pub async fn listen_for_restarts(component: String) {
    tokio::spawn(async move {
        listen_for_restarts_task(component).await.ok();
        exit(0);
    });
}
pub fn hash_password(password: String) -> Result<String, Box<dyn Error>> {
    let salt = SaltString::generate(OsRng);
    let params = Params::new(128 * 1024, 1, 1, Some(64)).or(Err("Invalid argon2 parameters"))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let hash = argon.hash_password(password.as_bytes(), &salt).or(Err("Couldn't hash password"))?;
    Ok(hash.to_string())
}
