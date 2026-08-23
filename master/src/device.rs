use crate::code::LUA_OS_RELEASE;
use diesel::prelude::*;
use diesel_async::pooled_connection::deadpool::Pool;
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use shared_api::{CommandLanguage, CommandOrigin};
use shared_server::models::DeviceInfo;
use shared_server::schema::{device_info, devices};
use shared_server::{
    models::{Command, Device},
    notify_server,
    schema::commands,
};
use std::{error::Error, time::SystemTime};

const DEVICE_INFO_TYPES: [(&str, CommandLanguage, &str); 1] = [("linux-os-release", CommandLanguage::Lua, LUA_OS_RELEASE)];

pub async fn process_devices(pool: Pool<AsyncPgConnection>) -> Result<(), Box<dyn Error>> {
    let mut db = pool.get().await?;
    let devices: Vec<Device> = devices::table.filter(devices::approved.eq(true)).get_results(&mut db).await?;
    let mut tasks = Vec::new();
    for device in devices {
        let pool = pool.clone();
        tasks.push(tokio::spawn(async {
            if let Err(e) = process_device(device, pool).await {
                eprintln!("Error while processing devices: {}", e);
            }
        }));
    }
    for i in tasks {
        if let Err(e) = i.await {
            eprintln!("Error while processing devices: {}", e);
        }
    }
    Ok(())
}
async fn process_device(device: Device, pool: Pool<AsyncPgConnection>) -> Result<(), Box<dyn Error>> {
    let mut db = pool.get().await?;
    let device_id = device.id.ok_or("Invalid device")?;
    let mut added_cmd = false;
    db.transaction(async |db| {
        let completed_commands: Vec<Command> = commands::table
            .filter(
                commands::device
                    .eq(device_id)
                    .and(commands::completed.eq(true).and(commands::origin.eq(CommandOrigin::SystemIdentification as i32))),
            )
            .get_results(db)
            .await?;
        for i in completed_commands {
            diesel::delete(
                device_info::table.filter(
                    device_info::device
                        .eq(device_id)
                        .and(device_info::info_type.eq(&i.device_info_type.clone().ok_or(diesel::result::Error::NotFound)?)),
                ),
            )
            .execute(db)
            .await?;
            diesel::insert_into(device_info::table)
                .values(DeviceInfo {
                    device: device_id,
                    info_type: i.device_info_type.ok_or(diesel::result::Error::NotFound)?,
                    success: i.status_code == 0,
                    data: i.stdout,
                    updated_timestamp: i.completed_timestamp.ok_or(diesel::result::Error::NotFound)?,
                })
                .execute(db)
                .await?;
            diesel::delete(commands::table.find(i.id.ok_or(diesel::result::Error::NotFound)?)).execute(db).await?;
        }
        let pending_commands: i64 = commands::table
            .filter(
                commands::device
                    .eq(device_id)
                    .and(commands::completed.eq(false))
                    .and(commands::origin.eq(CommandOrigin::SystemIdentification as i32)),
            )
            .count()
            .get_result(db)
            .await?;
        if pending_commands != 0 {
            return Ok(());
        }
        for i in DEVICE_INFO_TYPES {
            let device_info: Option<DeviceInfo> = device_info::table
                .filter(device_info::device.eq(device_id).and(device_info::info_type.eq(i.0)))
                .first(db)
                .await
                .optional()?;
            if let Some(device_info) = device_info
                && SystemTime::now().duration_since(device_info.updated_timestamp).or(Err(diesel::result::Error::NotFound))?.as_secs() < 60 * 60 * 24
            {
                continue;
            }
            let command = Command {
                id: None,
                device: device_id,
                code: i.2.to_string(),
                completed: false,
                status_code: 0,
                stdout: "".to_owned(),
                stderr: "".to_owned(),
                requested_timestamp: SystemTime::now(),
                completed_timestamp: None,
                origin: CommandOrigin::SystemIdentification as i32,
                device_info_type: Some(i.0.to_string()),
                uid: None,
                initiated: false,
                chat_id: None,
                chat_sequence: None,
                chat_tool_call_id: None,
                language: i.1 as i32,
                transient: false,
            };
            diesel::insert_into(commands::table).values(command).execute(db).await?;
            added_cmd = true;
        }
        Ok::<(), diesel::result::Error>(())
    })
    .await?;
    if added_cmd {
        notify_server(&format!("API:DEVICE:{}", device_id)).await?;
    }
    Ok(())
}
