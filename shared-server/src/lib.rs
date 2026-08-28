#![forbid(unsafe_code)]

use crate::{
    models::{Setting, User, UserSetting},
    schema::{settings, user_settings, users},
};
use argon2::{Algorithm, Argon2, Params, PasswordHash, PasswordHasher, PasswordVerifier, Version};
use aws_lc_rs::signature::{ED25519, ParsedPublicKey, UnparsedPublicKey};
use base64::{Engine, prelude::BASE64_STANDARD};
use diesel::prelude::*;
use diesel_async::{
    AsyncPgConnection,
    pooled_connection::{AsyncDieselConnectionManager, deadpool::Pool},
};
use diesel_async::{RunQueryDsl, pooled_connection::ManagerConfig};
use futures_util::FutureExt;
use jsonwebtoken::{DecodingKey, Validation, decode};
use rand::random_range;
use redis::{AsyncTypedCommands, Msg, PushInfo, PushKind, RedisError, aio::ConnectionManagerConfig};
use rustls::{
    ClientConfig, RootCertStore,
    pki_types::{CertificateDer, pem::PemObject},
};
use serde::{Deserialize, Serialize};
use std::{env, error::Error, process::exit, sync::OnceLock, time::Duration};
use tokio::{
    sync::{
        Semaphore,
        mpsc::{UnboundedReceiver, unbounded_channel},
    },
    task::spawn_blocking,
    time::sleep,
};
use tokio_postgres_rustls::MakeRustlsConnect;
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
static HASHING_SEMAPHORE: Semaphore = Semaphore::const_new(5);

pub async fn get_db_pool() -> Result<Pool<AsyncPgConnection>, Box<dyn Error>> {
    let sslmode = if cfg!(debug_assertions) { "disable" } else { "require" };
    let url = format!(
        "postgres://{}:{}@{}/{}?sslmode={}",
        env::var("DATABASE_USER").unwrap_or("app".to_string()),
        env::var("DATABASE_PASSWORD").unwrap_or("".to_string()),
        env::var("DATABASE_HOST").unwrap_or("localhost".to_string()),
        env::var("DATABASE_DB").unwrap_or("app".to_string()),
        sslmode
    );
    let mut cert_store = RootCertStore::empty();
    if let Ok(cert) = env::var("DATABASE_CERT") {
        let certificate = CertificateDer::from_pem_reader(cert.trim().as_bytes())?;
        cert_store.add(certificate)?;
    }
    let tls_config = ClientConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
        .with_root_certificates(cert_store)
        .with_no_client_auth();
    let mut manager_config = ManagerConfig::default();
    manager_config.custom_setup = Box::new(move |url| {
        let pg_config = MakeRustlsConnect::new(tls_config.clone());
        async {
            let (client, connection) = tokio_postgres::connect(url, pg_config).await.map_err(|e| ConnectionError::BadConnection(e.to_string()))?;
            AsyncPgConnection::try_from_client_and_connection(client, connection).await
        }
        .boxed()
    });
    let connection_manager = AsyncDieselConnectionManager::new_with_config(url, manager_config);
    let pool = Pool::builder(connection_manager).build()?;
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
pub async fn hash_password(password: String) -> Result<String, Box<dyn Error>> {
    let params = Params::new(128 * 1024, 1, 1, Some(64)).or(Err("Invalid argon2 parameters"))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let lock = HASHING_SEMAPHORE.acquire().await?;
    let hash = spawn_blocking(move || {
        let hash = argon.hash_password(password.as_bytes()).or(Err("Couldn't hash password"))?;
        drop(lock);
        Ok::<String, String>(hash.to_string())
    })
    .await??;
    Ok(hash)
}
pub async fn verify_password(user_password: String, attempt_password: String) -> Result<(), Box<dyn Error>> {
    let lock = HASHING_SEMAPHORE.acquire().await?;
    spawn_blocking(move || {
        let hash = PasswordHash::new(&user_password).map_err(|e| e.to_string())?;
        Argon2::default().verify_password(attempt_password.as_bytes(), &hash).map_err(|e| e.to_string())?;
        drop(lock);
        Ok::<(), String>(())
    })
    .await??;
    Ok(())
}
pub fn parse_pubkey(pubkey: String) -> Result<ParsedPublicKey, Box<dyn Error>> {
    let key = UnparsedPublicKey::new(&ED25519, BASE64_STANDARD.decode(pubkey)?).parse()?;
    Ok(key)
}
pub async fn authenticate_jwt(jwt: &str, aud: &str, db: &mut AsyncPgConnection) -> Result<(User, Option<ParsedPublicKey>), Box<dyn Error>> {
    if JWT_KEY.get().is_none() {
        let jwt_secret: Setting = settings::table.find("jwt_secret").first(db).await?;
        let jwt_secret = BASE64_STANDARD.decode(jwt_secret.value)?;
        JWT_KEY.set(jwt_secret).ok();
    }
    let key = DecodingKey::from_secret(JWT_KEY.get().ok_or("Invalid JWT key")?);
    let mut validation = Validation::new(jsonwebtoken::Algorithm::HS512);
    validation.validate_nbf = true;
    validation.set_audience(&[aud]);
    validation.set_required_spec_claims(&["aud", "sub", "nbf", "exp", "counter"]);
    let token = decode::<Claims>(jwt, &key, &validation)?;
    let user: User = users::table.find(token.claims.sub).first(db).await?;
    if !user.enabled || user.jwt_counter as usize != token.claims.counter {
        return Err("Wrong JWT counter or user isn't enabled".into());
    }
    let mut pubkey = None;
    if let Some(pubkey_claim) = token.claims.pubkey {
        pubkey = Some(parse_pubkey(pubkey_claim)?);
    }
    Ok((user, pubkey))
}
pub async fn get_user_setting(uid: Uuid, name: String, db: &mut AsyncPgConnection) -> Result<UserSetting, Box<dyn Error>> {
    let setting: UserSetting = user_settings::table
        .filter(user_settings::uid.eq(uid).and(user_settings::name.eq(&name)))
        .first(db)
        .await
        .optional()?
        .unwrap_or(UserSetting { uid, name, value: "".to_string() });
    Ok(setting)
}
