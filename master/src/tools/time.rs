use rig::{
    completion::ToolDefinition,
    tool::{Tool, ToolError},
};
use serde::Deserialize;
use serde_json::json;
use time::OffsetDateTime;

pub struct TimeTool;

#[derive(Deserialize)]
pub struct TimeArgs {}

impl Tool for TimeTool {
    const NAME: &str = "time";
    type Error = ToolError;
    type Args = TimeArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: "Get current time".into(),
            parameters: json!({
              "type": "object",
              "properties": {
                "dummy": {"type": "string", "description": "Dummy argument"},
              }
            }),
        }
    }
    async fn call(&self, _: Self::Args) -> Result<Self::Output, Self::Error> {
        Ok(OffsetDateTime::now_utc().to_string())
    }
}
