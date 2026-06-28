use crate::DB_POOL;
use diesel::prelude::*;
use rig_core::{
    completion::ToolDefinition,
    tool::{Tool, ToolError},
};
use serde::Deserialize;
use serde_json::json;
use shared_server::schema::messages;
use uuid::Uuid;

pub struct CompactTool {
    pub chat: Uuid,
    pub sequence: i32,
}

#[derive(Deserialize)]
pub struct CompactArgs {
    message_count: u64,
}

impl Tool for CompactTool {
    const NAME: &str = "compact";
    type Error = ToolError;
    type Args = CompactArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: "Remove the previous N messages from context. This should be used to reduce context length for repetitive messages".into(),
            parameters: json!({
              "type": "object",
              "properties": {
                "message_count": {"type": "number", "description": "The number of messages to remove from context. This does not include the current message. Ensure this isn't so large as to remove important context."},
              },
              "required": ["message_count"]
            }),
        }
    }
    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let pool = DB_POOL.get().ok_or(ToolError::ToolCallError("Database connection failed".into()))?;
        let mut db = pool.get().map_err(|_| ToolError::ToolCallError("Database connection failed".into()))?;
        diesel::update(
            messages::table.filter(
                messages::chat
                    .eq(self.chat)
                    .and(messages::sequence.lt(self.sequence))
                    .and(messages::sequence.ge(self.sequence - args.message_count as i32)),
            ),
        )
        .set(messages::compacted.eq(true))
        .execute(&mut db)
        .map_err(|_| ToolError::ToolCallError("Updating messages failed".into()))?;
        Ok("OK".into())
    }
}
