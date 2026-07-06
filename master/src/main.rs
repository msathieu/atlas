#![forbid(unsafe_code)]

use crate::device::process_devices;
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
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::{PgConnection, QueryDsl, RunQueryDsl};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use rcgen::{CertificateParams, KeyPair, PKCS_ECDSA_P521_SHA512};
use shared_api::{CommandOrigin, print_startup};
use shared_server::models::{Setting, User};
use shared_server::schema::{commands, settings, users};
use shared_server::{get_db_pool, get_valkey_client, hash_password, listen_for_restarts, notify_server, wait_for_pubsub_msg};
use std::error::Error;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::time::{sleep, timeout};

mod device;
mod llm;
mod lua;
mod timers;
mod tool;
mod tools;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("../shared-server/migrations");
static DB_POOL: OnceLock<Pool<ConnectionManager<PgConnection>>> = OnceLock::new();

#[tokio::main]
async fn main() {
    print_startup("master", env!("CARGO_PKG_VERSION"));
    dotenvy::dotenv().ok();
    notify_server("MASTER:RESTART").await.unwrap();
    listen_for_restarts("MASTER".to_string()).await;
    let pool = get_db_pool().unwrap();
    {
        let mut db = pool.get().unwrap();
        db.run_pending_migrations(MIGRATIONS).unwrap();
        create_settings(&mut db);
        diesel::delete(commands::table.filter(commands::origin.eq(CommandOrigin::LLMTool as i32))).execute(&mut db).unwrap();
    }
    let device_pool = pool.clone();
    let llm_pool = pool.clone();
    let tool_pool = pool.clone();
    let timers_pool = pool.clone();
    DB_POOL.set(pool).unwrap();
    tokio::spawn(async { tasks_loop_wrapper(device_pool, "MASTER:DEVICES", process_devices).await });
    tokio::spawn(async { tasks_loop_wrapper(llm_pool, "MASTER:LLM", process_chats).await });
    tokio::spawn(async { tasks_loop_wrapper(tool_pool, "MASTER:TOOLS", process_tool_calls).await });
    tokio::spawn(async { tasks_loop_wrapper(timers_pool, "MASTER:TIMERS", process_timers).await });
    loop {
        sleep(Duration::from_hours(24)).await;
    }
}
async fn tasks_loop_wrapper<T: Future<Output = Result<(), Box<dyn Error>>>>(pool: Pool<ConnectionManager<PgConnection>>, sub_channel: &str, func: fn(Pool<ConnectionManager<PgConnection>>) -> T) {
    loop {
        tasks_loop(pool.clone(), sub_channel, func).await.ok();
    }
}
async fn tasks_loop<T: Future<Output = Result<(), Box<dyn Error>>>>(
    pool: Pool<ConnectionManager<PgConnection>>,
    sub_channel: &str,
    func: fn(Pool<ConnectionManager<PgConnection>>) -> T,
) -> Result<(), Box<dyn Error>> {
    let (mut pubsub_client, mut receiver) = get_valkey_client().await?;
    pubsub_client.subscribe(sub_channel).await?;
    loop {
        if let Err(e) = func(pool.clone()).await {
            println!("Error while running tasks: {}", e)
        }
        timeout(Duration::from_secs(300), async { wait_for_pubsub_msg(&mut receiver).await }).await.ok();
    }
}
fn does_setting_exist(db: &mut PgConnection, name: &str) -> Result<bool, Box<dyn Error>> {
    let setting_exists = settings::table.find(name).first::<Setting>(db).optional()?.is_some();
    Ok(setting_exists)
}
fn create_settings(db: &mut PgConnection) {
    db.build_transaction()
        .serializable()
        .run(|db| {
            if !does_setting_exist(db, "allow_new_devices")? {
                diesel::insert_into(settings::table)
                    .values(Setting {
                        name: "allow_new_devices".to_owned(),
                        value: "true".to_string(),
                    })
                    .execute(db)?;
            }
            if !does_setting_exist(db, "apiserver_host")? {
                diesel::insert_into(settings::table)
                    .values(Setting {
                        name: "apiserver_host".to_owned(),
                        value: "localhost".to_string(),
                    })
                    .execute(db)?;
            }
            if !does_setting_exist(db, "webserver_host")? {
                diesel::insert_into(settings::table)
                    .values(Setting {
                        name: "webserver_host".to_owned(),
                        value: "localhost".to_string(),
                    })
                    .execute(db)?;
            }
            if !does_setting_exist(db, "llm_default_provider")? {
                diesel::insert_into(settings::table)
                    .values(Setting {
                        name: "llm_default_provider".to_owned(),
                        value: "openrouter".to_string(),
                    })
                    .execute(db)?;
            }
            if !does_setting_exist(db, "llm_generic_endpoint")? {
                diesel::insert_into(settings::table)
                    .values(Setting {
                        name: "llm_generic_endpoint".to_owned(),
                        value: "".to_string(),
                    })
                    .execute(db)?;
            }
            if !does_setting_exist(db, "llm_generic_key")? {
                diesel::insert_into(settings::table)
                    .values(Setting {
                        name: "llm_generic_key".to_owned(),
                        value: "".to_string(),
                    })
                    .execute(db)?;
            }
            if !does_setting_exist(db, "llm_generic_model")? {
                diesel::insert_into(settings::table)
                    .values(Setting {
                        name: "llm_generic_model".to_owned(),
                        value: "".to_string(),
                    })
                    .execute(db)?;
            }
            if !does_setting_exist(db, "llm_openrouter_key")? {
                diesel::insert_into(settings::table)
                    .values(Setting {
                        name: "llm_openrouter_key".to_owned(),
                        value: "".to_string(),
                    })
                    .execute(db)?;
            }
            if !does_setting_exist(db, "llm_openrouter_model")? {
                diesel::insert_into(settings::table)
                    .values(Setting {
                        name: "llm_openrouter_model".to_owned(),
                        value: "z-ai/glm-5.2".to_string(),
                    })
                    .execute(db)?;
            }
            if !does_setting_exist(db, "mail_server")? {
                diesel::insert_into(settings::table)
                    .values(Setting {
                        name: "mail_server".to_owned(),
                        value: "".to_string(),
                    })
                    .execute(db)?;
            }
            if !does_setting_exist(db, "mail_username")? {
                diesel::insert_into(settings::table)
                    .values(Setting {
                        name: "mail_username".to_owned(),
                        value: "".to_string(),
                    })
                    .execute(db)?;
            }
            if !does_setting_exist(db, "mail_password")? {
                diesel::insert_into(settings::table)
                    .values(Setting {
                        name: "mail_password".to_owned(),
                        value: "".to_string(),
                    })
                    .execute(db)?;
            }
            if !does_setting_exist(db, "mail_from")? {
                diesel::insert_into(settings::table)
                    .values(Setting {
                        name: "mail_from".to_owned(),
                        value: "".to_string(),
                    })
                    .execute(db)?;
            }
            if !does_setting_exist(db, "mail_to")? {
                diesel::insert_into(settings::table)
                    .values(Setting {
                        name: "mail_to".to_owned(),
                        value: "".to_string(),
                    })
                    .execute(db)?;
            }
            if !does_setting_exist(db, "tls_key")? {
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
                    .execute(db)?;
                diesel::insert_into(settings::table)
                    .values(Setting {
                        name: "tls_cert".to_owned(),
                        value: cert_pem,
                    })
                    .execute(db)?;
            }
            if !does_setting_exist(db, "jwt_secret")? {
                let jwt_secret: [u8; 64] = rand::random();
                diesel::insert_into(settings::table)
                    .values(Setting {
                        name: "jwt_secret".to_owned(),
                        value: BASE64_STANDARD.encode(jwt_secret),
                    })
                    .execute(db)?;
            }
            if !does_setting_exist(db, "registration_token")? {
                let registration_token: [u8; 64] = rand::random();
                diesel::insert_into(settings::table)
                    .values(Setting {
                        name: "registration_token".to_owned(),
                        value: BASE64_STANDARD.encode(registration_token),
                    })
                    .execute(db)?;
            }
            if !does_setting_exist(db, "web_encryption_key")? {
                let web_encryption_key = PrivateDecryptingKey::generate(KeySize::Rsa8192)?.as_der()?;
                diesel::insert_into(settings::table)
                    .values(Setting {
                        name: "web_encryption_key".to_owned(),
                        value: BASE64_STANDARD.encode(web_encryption_key.as_ref()),
                    })
                    .execute(db)?;
            }
            if !does_setting_exist(db, "web_signing_key")? {
                let web_signing_key = Ed25519KeyPair::generate()?.to_pkcs8()?;
                diesel::insert_into(settings::table)
                    .values(Setting {
                        name: "web_signing_key".to_owned(),
                        value: BASE64_STANDARD.encode(web_signing_key.as_ref()),
                    })
                    .execute(db)?;
            }
            Ok::<(), Box<dyn Error>>(())
        })
        .unwrap();
    db.build_transaction()
        .serializable()
        .run(|db| {
            let users_count: i64 = users::table.count().get_result(db)?;
            if users_count != 0 {
                return Ok(());
            }
            println!("No users found, creating admin account...");
            let password: [u8; 64] = rand::random();
            let password = BASE64_STANDARD.encode(password);
            let hash = hash_password(password.clone())?;
            diesel::insert_into(users::table)
                .values(User {
                    id: None,
                    username: "admin".to_owned(),
                    password: hash,
                    admin: true,
                    jwt_counter: 0,
                })
                .execute(db)?;
            println!("Initial Credentials: admin / {}", password);
            Ok::<(), Box<dyn Error>>(())
        })
        .unwrap();
}
