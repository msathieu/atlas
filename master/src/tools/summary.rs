use crate::DB_POOL;
use diesel::prelude::*;
use rig_core::tool::{Tool, ToolError};
use serde::Deserialize;
use serde_json::json;
use shared_server::schema::chats;
use uuid::Uuid;

pub struct SummaryTool {
    pub chat: Uuid,
}

#[derive(Deserialize)]
pub struct SummaryArgs {
    summary: String,
}

impl Tool for SummaryTool {
    const NAME: &str = "summary";
    type Error = ToolError;
    type Args = SummaryArgs;
    type Output = String;

    fn description(&self) -> String {
        "Set the summary of the conversation".into()
    }
    fn parameters(&self) -> serde_json::Value {
        json!({
        "type": "object",
            "properties": {
              "summary": {"type": "string", "description": "The summary of the conversation, this should fit in one line."}
            },
            "required": ["summary"]
          })
    }
    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let pool = DB_POOL.get().ok_or(ToolError::ToolCallError("Database connection failed".into()))?;
        let mut db = pool.get().map_err(|_| ToolError::ToolCallError("Database connection failed".into()))?;
        diesel::update(chats::table.find(self.chat))
            .set(chats::summary.eq(args.summary))
            .execute(&mut db)
            .map_err(|_| ToolError::ToolCallError("Updating chat failed".into()))?;
        Ok("OK".into())
    }
}
