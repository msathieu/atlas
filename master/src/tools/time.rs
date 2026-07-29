use rig_core::tool::{PortableTool, ToolExecutionError};
use serde::Deserialize;
use serde_json::json;
use time::OffsetDateTime;

pub struct TimeTool;

#[derive(Deserialize)]
pub struct TimeArgs {}

impl PortableTool for TimeTool {
    const NAME: &str = "time";
    type Error = ToolExecutionError;
    type Args = TimeArgs;
    type Output = String;

    fn description(&self) -> String {
        "Get current time".into()
    }
    fn parameters(&self) -> serde_json::Value {
        json!({
          "type": "object",
          "properties": {
            "dummy": {"type": "string", "description": "Dummy argument"},
          }
        })
    }
    async fn call(&self, _: Self::Args) -> Result<Self::Output, Self::Error> {
        Ok(OffsetDateTime::now_utc().to_string())
    }
}
