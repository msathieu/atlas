use crate::DB_POOL;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use rig_core::tool::{PortableTool, ToolExecutionError};
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

impl PortableTool for SummaryTool {
    const NAME: &str = "summary";
    type Error = ToolExecutionError;
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
        let pool = DB_POOL.get().ok_or(ToolExecutionError::other("Database connection failed"))?;
        let mut db = pool.get().await.map_err(|_| ToolExecutionError::other("Database connection failed"))?;
        diesel::update(chats::table.find(self.chat))
            .set(chats::summary.eq(args.summary))
            .execute(&mut db)
            .await
            .map_err(|_| ToolExecutionError::other("Updating chat failed"))?;
        Ok("OK".into())
    }
}
