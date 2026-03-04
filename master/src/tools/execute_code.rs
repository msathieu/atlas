use crate::DB_POOL;
use diesel::prelude::*;
use fred::prelude::{ClientLike, EventInterface, PubsubInterface};
use rig::{
    completion::ToolDefinition,
    tool::{Tool, ToolError},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use shared_api::CommandOrigin;
use shared_server::{get_sub_client, models::Command, notify_server, schema::commands};
use std::{error::Error, time::SystemTime};
use uuid::Uuid;

pub struct ExecuteCodeTool {
    pub user: Uuid,
}
#[derive(Deserialize)]
pub struct ExecuteCodeArgs {
    device: String,
    code: String,
    return_stdout: bool,
    return_stderr: bool,
}
#[derive(Serialize)]
pub struct ExecuteCodeOutput {
    status_code: i32,
    stdout: String,
    stderr: String,
}

impl ExecuteCodeTool {
    async fn call_internal(&self, args: ExecuteCodeArgs) -> Result<ExecuteCodeOutput, Box<dyn Error>> {
        let pool = DB_POOL.get().ok_or("Database connection failed")?;
        let mut db = pool.get()?;
        let device_uuid = Uuid::parse_str(&args.device)?;
        let command = Command {
            id: None,
            device: device_uuid,
            uid: Some(self.user),
            code: args.code,
            completed: true,
            status_code: 0,
            stdout: "".to_owned(),
            stderr: "".to_owned(),
            requested_timestamp: SystemTime::now(),
            completed_timestamp: None,
            origin: CommandOrigin::LLMTool as i32,
            origin_id: "".to_string(),
            initiated: false,
        };
        let cmd: Command = diesel::insert_into(commands::table).values(command).get_result(&mut db)?;
        let cmd_id = cmd.id.ok_or("Invalid command")?;
        let sub_client = get_sub_client().await?;
        sub_client.subscribe(format!("MASTER:CMD:{}", cmd_id)).await?;
        let mut receiver = sub_client.message_rx();
        diesel::update(commands::table.find(cmd_id)).set(commands::completed.eq(false)).execute(&mut db)?;
        notify_server(&format!("API:DEVICE:{}", device_uuid)).await?;
        receiver.recv().await?;
        sub_client.quit().await?;
        let cmd: Command = commands::table.find(cmd_id).get_result(&mut db)?;
        diesel::delete(commands::table.find(cmd_id)).execute(&mut db)?;
        let stdout = if args.return_stdout { cmd.stdout } else { "".to_string() };
        let stderr = if args.return_stderr { cmd.stderr } else { "".to_string() };
        Ok(ExecuteCodeOutput {
            status_code: cmd.status_code,
            stdout,
            stderr,
        })
    }
}
impl Tool for ExecuteCodeTool {
    const NAME: &str = "execute_code";
    type Error = ToolError;
    type Args = ExecuteCodeArgs;
    type Output = ExecuteCodeOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: "Execute code on a server. This tool requires approval".into(),
            parameters: json!({
              "type": "object",
              "properties": {
                "device": {"type": "string", "description": "The server to execute code on"},
                "code": {"type": "string", "description": "The code to execute. This must be Lua 5.5 code, if you want to retrieve information, ensure it is printed to stdout or stderr"},
                "return_stdout": {"type": "boolean", "description": "Include stdout in the tool response"},
                "return_stderr": {"type": "boolean", "description": "Include stderr in the tool response"}
              },
              "required": ["device", "code", "return_stdout", "return_stderr"]
            }),
        }
    }
    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        self.call_internal(args).await.map_err(|e| ToolError::ToolCallError(e.to_string().into()))
    }
}
