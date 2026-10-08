#![forbid(unsafe_code)]

use crate::containers::process_containers;
use crate::device::process_devices;
use crate::forge::process_forge_actions;
use crate::hsm::process_hsm_requests;
use crate::llm::process_chats;
use crate::migrations::run_migrations;
use crate::settings::create_settings;
use crate::timers::process_timers;
use crate::tool::process_tool_calls;
use diesel::prelude::*;
use diesel_async::async_connection_wrapper::AsyncConnectionWrapper;
use diesel_async::pooled_connection::deadpool::{Object, Pool};
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use shared_api::{CommandOrigin, print_startup};
use shared_server::schema::{commands, tool_calls};
use shared_server::{get_db_pool, get_valkey_client, listen_for_restarts, notify_server, wait_for_pubsub_msg};
use std::error::Error;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::task::spawn_blocking;
use tokio::time::{sleep, timeout};

mod code;
mod containers;
mod device;
mod forge;
mod hsm;
mod llm;
mod migrations;
mod settings;
mod timers;
mod tool;
mod tools;
mod tpm;

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
        })
        .await
        .unwrap();
        db = pool.get().await.unwrap();
        create_settings(&mut db).await.unwrap();
        diesel::delete(commands::table.filter(commands::origin.eq(CommandOrigin::LLMTool as i32)))
            .execute(&mut db)
            .await
            .unwrap();
        diesel::update(tool_calls::table.filter(tool_calls::initiated.eq(true).and(tool_calls::response.is_null())))
            .set((tool_calls::response.eq(Some("Tool call was interrupted")), tool_calls::error.eq(true)))
            .execute(&mut db)
            .await
            .unwrap();
        run_migrations(&mut db).await.unwrap();
    }
    let device_pool = pool.clone();
    let llm_pool = pool.clone();
    let tool_pool = pool.clone();
    let timers_pool = pool.clone();
    let container_pool = pool.clone();
    let forge_pool = pool.clone();
    let hsm_pool = pool.clone();
    DB_POOL.set(pool).ok();
    tokio::spawn(async { tasks_loop_wrapper(device_pool, "MASTER:DEVICES", process_devices, Duration::from_secs(300)).await });
    tokio::spawn(async { tasks_loop_wrapper(llm_pool, "MASTER:LLM", process_chats, Duration::from_secs(300)).await });
    tokio::spawn(async { tasks_loop_wrapper(tool_pool, "MASTER:TOOLS", process_tool_calls, Duration::from_secs(300)).await });
    tokio::spawn(async { tasks_loop_wrapper(timers_pool, "MASTER:TIMERS", process_timers, Duration::from_secs(300)).await });
    tokio::spawn(async { tasks_loop_wrapper(container_pool, "MASTER:CONTAINERS", process_containers, Duration::from_secs(10)).await });
    tokio::spawn(async { tasks_loop_wrapper(forge_pool, "MASTER:FORGE", process_forge_actions, Duration::from_secs(60)).await });
    tokio::spawn(async { tasks_loop_wrapper(hsm_pool, "MASTER:HSM", process_hsm_requests, Duration::from_secs(10)).await });
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
