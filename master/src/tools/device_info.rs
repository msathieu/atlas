use crate::DB_POOL;
use diesel::prelude::*;
use rig_core::{
    completion::ToolDefinition,
    tool::{Tool, ToolError},
};
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

impl Tool for DeviceInfoTool {
    const NAME: &str = "device_info";
    type Error = ToolError;
    type Args = DeviceInfoArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: "Get information about the target server".into(),
            parameters: json!({
              "type": "object",
              "properties": {
                "device": {"type": "string", "description": "The server to get information about"},
                "info_type": {"type": "string", "description": "Which kind of information you want, possible values are: linux-os-release"}
              },
              "required": ["device", "info_type"]
            }),
        }
    }
    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let pool = DB_POOL.get().ok_or(ToolError::ToolCallError("Database connection failed".into()))?;
        let mut db = pool.get().map_err(|_| ToolError::ToolCallError("Database connection failed".into()))?;
        let device_uuid = Uuid::parse_str(&args.device).map_err(|_| ToolError::ToolCallError("Invalid device".into()))?;
        let info: DeviceInfo = device_info::table
            .filter(device_info::device.eq(device_uuid).and(device_info::info_type.eq(args.info_type)))
            .get_result(&mut db)
            .map_err(|_| ToolError::ToolCallError("Invalid device info".into()))?;
        if !info.success {
            return Err(ToolError::ToolCallError("Obtaining device info failed".into()));
        }
        Ok(info.data)
    }
}
