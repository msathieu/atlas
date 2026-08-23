use crate::DB_POOL;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use rig_core::tool::{PortableTool, ToolExecutionError};
use serde::Deserialize;
use serde_json::json;
use shared_server::{models::DeviceInfo, schema::device_info};
use uuid::Uuid;

pub struct DeviceInfoTool;

#[derive(Deserialize)]
pub struct DeviceInfoArgs {
    device: String,
    info_type: String,
}

impl PortableTool for DeviceInfoTool {
    const NAME: &str = "device_info";
    type Error = ToolExecutionError;
    type Args = DeviceInfoArgs;
    type Output = String;

    fn description(&self) -> String {
        "Get information about the target server".into()
    }
    fn parameters(&self) -> serde_json::Value {
        json!({
          "type": "object",
          "properties": {
            "device": {"type": "string", "description": "The server to get information about"},
            "info_type": {"type": "string", "description": "Which kind of information you want, possible values are: linux-os-release"}
          },
          "required": ["device", "info_type"]
        })
    }
    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let pool = DB_POOL.get().ok_or(ToolExecutionError::other("Database connection failed"))?;
        let mut db = pool.get().await.map_err(|_| ToolExecutionError::other("Database connection failed"))?;
        let device_uuid = Uuid::parse_str(&args.device).map_err(|_| ToolExecutionError::other("Invalid device"))?;
        let info: DeviceInfo = device_info::table
            .filter(device_info::device.eq(device_uuid).and(device_info::info_type.eq(args.info_type)))
            .get_result(&mut db)
            .await
            .map_err(|_| ToolExecutionError::other("Invalid device info"))?;
        if !info.success {
            return Err(ToolExecutionError::other("Obtaining device info failed"));
        }
        Ok(info.data)
    }
}
