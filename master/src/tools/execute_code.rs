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
use shared_server::{
    get_sub_client,
    models::{Command, PreapprovedCode},
    notify_server,
    schema::{commands, preapproved_code},
};
use std::{error::Error, str::FromStr, time::SystemTime};
use uuid::Uuid;

pub struct ExecuteCodeTool {
    pub user: Uuid,
    pub chat: Uuid,
    pub sequence: i32,
    pub tool_call_id: String,
}
#[derive(Deserialize)]
pub struct ExecuteCodeArgs {
    device: String,
    code: Option<String>,
    preapproved_code_id: Option<String>,
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
        if (args.code.is_some() && args.preapproved_code_id.is_some()) || (args.code.is_none() && args.preapproved_code_id.is_none()) {
            return Err("Invalid tool call arguments".into());
        }
        let mut code = "".to_string();
        if let Some(code_arg) = args.code {
            code = code_arg;
        }
        let pool = DB_POOL.get().ok_or("Database connection failed")?;
        let mut db = pool.get()?;
        if let Some(preapproved_id) = args.preapproved_code_id {
            let preapproved_code: PreapprovedCode = preapproved_code::table.find(Uuid::from_str(&preapproved_id)?).get_result(&mut db)?;
            if preapproved_code.chat != self.chat {
                return Err("Invalid preapproved code".into());
            }
            code = preapproved_code.code;
        }
        let device_uuid = Uuid::parse_str(&args.device)?;
        let command = Command {
            id: None,
            device: device_uuid,
            uid: Some(self.user),
            code,
            completed: true,
            status_code: 0,
            stdout: "".to_owned(),
            stderr: "".to_owned(),
            requested_timestamp: SystemTime::now(),
            completed_timestamp: None,
            origin: CommandOrigin::LLMTool as i32,
            device_info_type: None,
            chat_id: Some(self.chat),
            chat_sequence: Some(self.sequence),
            chat_tool_call_id: Some(self.tool_call_id.clone()),
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
            description: "Execute code on a server. This tool requires approval unless preapproved code is used.".into(),
            parameters: json!({
              "type": "object",
              "properties": {
                "device": {"type": "string", "description": "The server to execute code on"},
                "code": {"type": "string", "description": "The code to execute. This must be Lua 5.5 code, if you want to retrieve information, ensure it is printed to stdout or stderr. Do not include this field if using preapproved code"},
                "preapproved_code_id": {"type": "string", "description": "The ID of previously approved code, this field is mutually exclusive with code"},
                "return_stdout": {"type": "boolean", "description": "Include stdout in the tool response"},
                "return_stderr": {"type": "boolean", "description": "Include stderr in the tool response"}
              },
              "required": ["device", "return_stdout", "return_stderr"]
            }),
        }
    }
    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        self.call_internal(args).await.map_err(|e| ToolError::ToolCallError(e.to_string().into()))
    }
}
