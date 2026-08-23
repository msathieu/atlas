use crate::DB_POOL;
use diesel_async::RunQueryDsl;
use rig_core::tool::{PortableTool, ToolExecutionError};
use serde::Deserialize;
use serde_json::json;
use shared_server::{models::PreapprovedCode, schema::preapproved_code};
use uuid::Uuid;

pub struct PreapproveCodeTool {
    pub chat: Uuid,
}

#[derive(Deserialize)]
pub struct PreapproveCodeArgs {
    pub code: String,
}

impl PortableTool for PreapproveCodeTool {
    const NAME: &str = "preapprove_code";
    type Error = ToolExecutionError;
    type Args = PreapproveCodeArgs;
    type Output = String;

    fn description(&self) -> String {
        "Preapprove code so it can be executed later without prompting the user. This tool requires approval".into()
    }
    fn parameters(&self) -> serde_json::Value {
        json!({
          "type": "object",
          "properties": {
            "code": {"type": "string", "description": "The code to preapprove. Reference the execute_code tool for instructions"},
          },
          "required": ["code"]
        })
    }
    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let pool = DB_POOL.get().ok_or(ToolExecutionError::other("Database connection failed"))?;
        let mut db = pool.get().await.map_err(|_| ToolExecutionError::other("Database connection failed"))?;
        let preapproved_code: PreapprovedCode = diesel::insert_into(preapproved_code::table)
            .values(PreapprovedCode {
                id: None,
                chat: self.chat,
                code: args.code,
            })
            .get_result(&mut db)
            .await
            .map_err(|_| ToolExecutionError::other("Failed to insert into db"))?;
        let code_id = preapproved_code.id.ok_or(ToolExecutionError::other("Invalid request"))?;
        Ok(code_id.to_string())
    }
}
