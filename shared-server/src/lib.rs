#![forbid(unsafe_code)]

use crate::{
    models::{Setting, User, UserSetting},
    schema::{settings, user_settings, users},
};
use argon2::{
    Algorithm, Argon2, Params, PasswordHasher, Version,
    password_hash::{SaltString, rand_core::OsRng},
};
use aws_lc_rs::signature::{ED25519, ParsedPublicKey, UnparsedPublicKey};
use base64::{Engine, prelude::BASE64_STANDARD};
use diesel::{
    PgConnection,
    r2d2::{ConnectionManager, Pool},
};
use diesel::{prelude::*, r2d2::PooledConnection};
use jsonwebtoken::{DecodingKey, Validation, decode};
use rand::random_range;
use redis::{AsyncTypedCommands, Msg, PushInfo, PushKind, RedisError, aio::ConnectionManagerConfig};
use serde::{Deserialize, Serialize};
use std::{env, error::Error, process::exit, sync::OnceLock, time::Duration};
use tokio::{
    sync::mpsc::{UnboundedReceiver, unbounded_channel},
    time::sleep,
};
use uuid::Uuid;

pub mod binary;
pub mod models;
pub mod schema;

#[derive(Clone, Deserialize, Serialize)]
pub struct Claims {
    pub aud: String,
    pub sub: Uuid,
    pub nbf: usize,
    pub exp: usize,
    pub counter: usize,
    pub pubkey: Option<String>,
}

static PUBSUB_POOL: OnceLock<redis::aio::ConnectionManager> = OnceLock::new();
static JWT_KEY: OnceLock<Vec<u8>> = OnceLock::new();

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
    config = config.set_automatic_resubscription().set_number_of_retries(12);
    let (tx, rx) = unbounded_channel();
    config = config.set_push_sender(tx);
    let manager = redis::aio::ConnectionManager::new_with_config(client, config).await?;
    Ok((manager, rx))
}
pub async fn notify_server(channel: &str) -> Result<(), Box<dyn Error>> {
    notify_server_with_msg(channel, "").await
}
pub async fn notify_server_with_msg(channel: &str, msg: &str) -> Result<(), Box<dyn Error>> {
    if PUBSUB_POOL.get().is_none() {
        let client = get_valkey_client().await?;
        PUBSUB_POOL.set(client.0).ok();
    }
    PUBSUB_POOL.get().ok_or("Failed to get pubsub client")?.clone().publish::<&str, &str>(channel, msg).await?;
    Ok(())
}
pub async fn wait_for_pubsub_msg(receiver: &mut UnboundedReceiver<PushInfo>) -> String {
    loop {
        let received = receiver.recv().await;
        if received.is_none() {
            return "".into();
        }
        if let Some(push_info) = received
            && push_info.kind == PushKind::Message
        {
            if let Some(msg) = Msg::from_push_info(push_info)
                && let Ok(payload) = msg.get_payload::<String>()
            {
                return payload;
            }
            return "".into();
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
        listen_for_restarts_task(component.clone()).await.ok();
        if component != "MASTER" {
            let sleep_secs = random_range(15..60);
            sleep(Duration::from_secs(sleep_secs)).await;
        }
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
pub fn parse_pubkey(pubkey: String) -> Result<ParsedPublicKey, Box<dyn Error>> {
    let key = UnparsedPublicKey::new(&ED25519, BASE64_STANDARD.decode(pubkey)?).parse()?;
    Ok(key)
}
pub fn authenticate_jwt(
    jwt: &str,
    aud: &str,
    mut db: Option<&mut PooledConnection<ConnectionManager<PgConnection>>>,
    skip_db_checks: bool,
) -> Result<(Option<User>, Option<ParsedPublicKey>), Box<dyn Error>> {
    if JWT_KEY.get().is_none() {
        let jwt_secret: Setting = settings::table.find("jwt_secret").first(db.as_deref_mut().ok_or("No DB instance passed")?)?;
        let jwt_secret = BASE64_STANDARD.decode(jwt_secret.value)?;
        JWT_KEY.set(jwt_secret).ok();
    }
    let key = DecodingKey::from_secret(JWT_KEY.get().ok_or("Invalid JWT key")?);
    let mut validation = Validation::new(jsonwebtoken::Algorithm::HS512);
    validation.validate_nbf = true;
    validation.set_audience(&[aud]);
    validation.set_required_spec_claims(&["aud", "sub", "nbf", "exp", "counter"]);
    let token = decode::<Claims>(jwt, &key, &validation)?;
    let mut user = None;
    if !skip_db_checks {
        let local_user: User = users::table.find(token.claims.sub).first(db.ok_or("No DB instance passed")?)?;
        if !local_user.enabled || local_user.jwt_counter as usize != token.claims.counter {
            return Err("Wrong JWT counter or user isn't enabled".into());
        }
        user = Some(local_user);
    }
    let mut pubkey = None;
    if let Some(pubkey_claim) = token.claims.pubkey {
        pubkey = Some(parse_pubkey(pubkey_claim)?);
    }
    Ok((user, pubkey))
}
pub fn get_user_setting(uid: Uuid, name: String, db: &mut PooledConnection<ConnectionManager<PgConnection>>) -> Result<UserSetting, Box<dyn Error>> {
    let setting: UserSetting = user_settings::table
        .filter(user_settings::uid.eq(uid).and(user_settings::name.eq(&name)))
        .first(db)
        .optional()?
        .unwrap_or(UserSetting { uid, name, value: "".to_string() });
    Ok(setting)
}
