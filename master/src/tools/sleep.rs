use rig_core::tool::{Tool, ToolError};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use tokio::time::sleep;

pub struct SleepTool;

#[derive(Deserialize)]
pub struct SleepArgs {
    seconds: u64,
}

impl Tool for SleepTool {
    const NAME: &str = "sleep";
    type Error = ToolError;
    type Args = SleepArgs;
    type Output = String;

    fn description(&self) -> String {
        "Sleep for a certain time".into()
    }
    fn parameters(&self) -> serde_json::Value {
        json!({
        "type": "object",
            "properties": {
              "seconds": {"type": "number", "description": "The amount of seconds to sleep for"},
            },
            "required": ["seconds"]
          })
    }
    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        sleep(Duration::from_secs(args.seconds)).await;
        Ok("OK".into())
    }
}
