#![forbid(unsafe_code)]

use crate::containers::process_containers;
use crate::device::process_devices;
use crate::forge::process_forge_actions;
use crate::llm::process_chats;
use crate::timers::process_timers;
use crate::tool::process_tool_calls;
use aws_lc_rs::encoding::AsDer;
use aws_lc_rs::rsa::{KeySize, PrivateDecryptingKey};
use aws_lc_rs::signature::Ed25519KeyPair;
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use diesel::OptionalExtension;
use diesel::prelude::*;
use diesel_async::async_connection_wrapper::AsyncConnectionWrapper;
use diesel_async::pooled_connection::deadpool::{Object, Pool};
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use rcgen::{CertificateParams, KeyPair, PKCS_ECDSA_P521_SHA512};
use shared_api::{CommandOrigin, print_startup};
use shared_server::models::{Setting, User};
use shared_server::schema::{commands, settings, tool_calls, users};
use shared_server::{get_db_pool, get_valkey_client, hash_password, listen_for_restarts, notify_server, wait_for_pubsub_msg};
use std::error::Error;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::task::spawn_blocking;
use tokio::time::{sleep, timeout};
use webauthn_rs::fake::{FakePasskeyDistribution, WebauthnFakeCredentialGenerator};

mod code;
mod containers;
mod device;
mod forge;
mod llm;
mod timers;
mod tool;
mod tools;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("../shared-server/migrations");
static DB_POOL: OnceLock<Pool<AsyncPgConnection>> = OnceLock::new();

#[tokio::main]
async fn main() {
    print_startup("master", env!("CARGO_PKG_VERSION"));
    dotenvy::dotenv().ok();
    notify_server("MASTER:RESTART").await.unwrap();
    listen_for_restarts("MASTER".to_string()).await;
    rustls::crypto::aws_lc_rs::default_provider().install_default().unwrap();
    let pool = get_db_pool().await.unwrap();
    {
        let mut db = pool.get().await.unwrap();
        spawn_blocking(|| {
            let mut wrapper: AsyncConnectionWrapper<Object<AsyncPgConnection>> = AsyncConnectionWrapper::from(db);
            wrapper.run_pending_migrations(MIGRATIONS).unwrap();
        });
        db = pool.get().await.unwrap();
        create_settings(&mut db).await.unwrap();
        diesel::delete(commands::table.filter(commands::origin.eq(CommandOrigin::LLMTool as i32)))
            .execute(&mut db)
            .await
            .unwrap();
        diesel::update(tool_calls::table.filter(tool_calls::initiated.eq(true).and(tool_calls::response.is_null())))
            .set(tool_calls::response.eq(Some("Tool call was interrupted")))
            .execute(&mut db)
            .await
            .unwrap();
    }
    let device_pool = pool.clone();
    let llm_pool = pool.clone();
    let tool_pool = pool.clone();
    let timers_pool = pool.clone();
    let container_pool = pool.clone();
    let forge_pool = pool.clone();
    DB_POOL.set(pool).ok();
    tokio::spawn(async { tasks_loop_wrapper(device_pool, "MASTER:DEVICES", process_devices, Duration::from_secs(300)).await });
    tokio::spawn(async { tasks_loop_wrapper(llm_pool, "MASTER:LLM", process_chats, Duration::from_secs(300)).await });
    tokio::spawn(async { tasks_loop_wrapper(tool_pool, "MASTER:TOOLS", process_tool_calls, Duration::from_secs(300)).await });
    tokio::spawn(async { tasks_loop_wrapper(timers_pool, "MASTER:TIMERS", process_timers, Duration::from_secs(300)).await });
    tokio::spawn(async { tasks_loop_wrapper(container_pool, "MASTER:CONTAINERS", process_containers, Duration::from_secs(10)).await });
    tokio::spawn(async { tasks_loop_wrapper(forge_pool, "MASTER:FORGE", process_forge_actions, Duration::from_secs(60)).await });
    loop {
        sleep(Duration::from_hours(24)).await;
    }
}
async fn tasks_loop_wrapper<T: Future<Output = Result<(), Box<dyn Error>>>>(pool: Pool<AsyncPgConnection>, sub_channel: &str, func: fn(Pool<AsyncPgConnection>) -> T, interval: Duration) {
    loop {
        tasks_loop(pool.clone(), sub_channel, func, interval).await.ok();
    }
}
async fn tasks_loop<T: Future<Output = Result<(), Box<dyn Error>>>>(
    pool: Pool<AsyncPgConnection>,
    sub_channel: &str,
    func: fn(Pool<AsyncPgConnection>) -> T,
    interval: Duration,
) -> Result<(), Box<dyn Error>> {
    let (mut pubsub_client, mut receiver) = get_valkey_client().await?;
    pubsub_client.subscribe(sub_channel).await?;
    loop {
        if let Err(e) = func(pool.clone()).await {
            eprintln!("Error while running tasks: {}", e)
        }
        timeout(interval, async { wait_for_pubsub_msg(&mut receiver).await }).await.ok();
    }
}
async fn does_setting_exist(db: &mut AsyncPgConnection, name: &str) -> Result<bool, Box<dyn Error>> {
    let setting_exists = settings::table.find(name).first::<Setting>(db).await.optional()?.is_some();
    Ok(setting_exists)
}
async fn create_settings(db: &mut AsyncPgConnection) -> Result<(), Box<dyn Error>> {
    if !does_setting_exist(db, "allow_new_devices").await? {
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "allow_new_devices".to_owned(),
                value: "true".to_string(),
            })
            .execute(db)
            .await?;
    }
    if !does_setting_exist(db, "llm_default_provider").await? {
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "llm_default_provider".to_owned(),
                value: "generic".to_string(),
            })
            .execute(db)
            .await?;
    }
    if !does_setting_exist(db, "llm_generic_endpoint").await? {
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "llm_generic_endpoint".to_owned(),
                value: "".to_string(),
            })
            .execute(db)
            .await?;
    }
    if !does_setting_exist(db, "llm_generic_key").await? {
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "llm_generic_key".to_owned(),
                value: "".to_string(),
            })
            .execute(db)
            .await?;
    }
    if !does_setting_exist(db, "llm_generic_model").await? {
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "llm_generic_model".to_owned(),
                value: "".to_string(),
            })
            .execute(db)
            .await?;
    }
    if !does_setting_exist(db, "llm_openrouter_key").await? {
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "llm_openrouter_key".to_owned(),
                value: "".to_string(),
            })
            .execute(db)
            .await?;
    }
    if !does_setting_exist(db, "llm_openrouter_model").await? {
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "llm_openrouter_model".to_owned(),
                value: "z-ai/glm-5.3-flash".to_string(),
            })
            .execute(db)
            .await?;
    }
    if !does_setting_exist(db, "mail_username").await? {
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "mail_username".to_owned(),
                value: "".to_string(),
            })
            .execute(db)
            .await?;
    }
    if !does_setting_exist(db, "mail_password").await? {
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "mail_password".to_owned(),
                value: "".to_string(),
            })
            .execute(db)
            .await?;
    }
    if !does_setting_exist(db, "mail_from").await? {
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "mail_from".to_owned(),
                value: "".to_string(),
            })
            .execute(db)
            .await?;
    }
    if !does_setting_exist(db, "jwt_secret").await? {
        let jwt_secret: [u8; 64] = rand::random();
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "jwt_secret".to_owned(),
                value: BASE64_STANDARD.encode(jwt_secret),
            })
            .execute(db)
            .await?;
    }
    if !does_setting_exist(db, "registration_token").await? {
        let registration_token: [u8; 64] = rand::random();
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "registration_token".to_owned(),
                value: BASE64_STANDARD.encode(registration_token),
            })
            .execute(db)
            .await?;
    }
    if !does_setting_exist(db, "web_encryption_key").await? {
        let web_encryption_key = PrivateDecryptingKey::generate(KeySize::Rsa8192)?.as_der()?;
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "web_encryption_key".to_owned(),
                value: BASE64_STANDARD.encode(web_encryption_key.as_ref()),
            })
            .execute(db)
            .await?;
    }
    if !does_setting_exist(db, "web_signing_key").await? {
        let web_signing_key = Ed25519KeyPair::generate()?.to_pkcs8()?;
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "web_signing_key".to_owned(),
                value: BASE64_STANDARD.encode(web_signing_key.as_ref()),
            })
            .execute(db)
            .await?;
    }
    if !does_setting_exist(db, "fake_passkey_key").await? {
        let fake_passkey_key = WebauthnFakeCredentialGenerator::<FakePasskeyDistribution>::new_hmac_key()?;
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "fake_passkey_key".to_owned(),
                value: BASE64_STANDARD.encode(fake_passkey_key),
            })
            .execute(db)
            .await?;
    }
    if !does_setting_exist(db, "apk_hash").await? {
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "apk_hash".to_owned(),
                value: "".to_owned(),
            })
            .execute(db)
            .await?;
    }
    db.build_transaction()
        .serializable()
        .run(async |db| {
            if !does_setting_exist(db, "tls_key").await? {
                println!("TLS key not found, generating...");
                let key_pair = KeyPair::generate_for(&PKCS_ECDSA_P521_SHA512)?;
                let mut cert_params = CertificateParams::new(vec!["apiserver".to_owned()])?;
                cert_params.key_identifier_method = rcgen::KeyIdMethod::Sha512;
                let cert = cert_params.self_signed(&key_pair)?;
                let cert_pem = cert.pem().trim().to_string();
                println!("{}", cert_pem);

                diesel::insert_into(settings::table)
                    .values(Setting {
                        name: "tls_key".to_owned(),
                        value: key_pair.serialize_pem().trim().to_string(),
                    })
                    .execute(db)
                    .await?;
                diesel::insert_into(settings::table)
                    .values(Setting {
                        name: "tls_cert".to_owned(),
                        value: cert_pem,
                    })
                    .execute(db)
                    .await?;
            }
            Ok::<(), Box<dyn Error>>(())
        })
        .await?;
    db.build_transaction()
        .serializable()
        .run(async |db| {
            let users_count: i64 = users::table.count().get_result(db).await?;
            if users_count != 0 {
                return Ok(());
            }
            println!("No users found, creating admin account...");
            let password: [u8; 64] = rand::random();
            let password = BASE64_STANDARD.encode(password);
            let hash = hash_password(password.clone()).await?;
            diesel::insert_into(users::table)
                .values(User {
                    id: None,
                    username: "admin".to_owned(),
                    password: hash,
                    admin: true,
                    jwt_counter: 0,
                    enabled: true,
                    password_enabled: true,
                    last_login: None,
                    email: None,
                })
                .execute(db)
                .await?;
            println!("Initial Credentials: admin / {}", password);
            Ok::<(), Box<dyn Error>>(())
        })
        .await?;
    Ok(())
}
