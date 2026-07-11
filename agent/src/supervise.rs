use std::{env, error::Error};
use tokio::process::Command;

pub async fn supervise_agent() -> Result<(), Box<dyn Error>> {
    let path = env::current_exe()?;
    loop {
        let mut cmd = Command::new(&path).spawn()?;
        cmd.wait().await?;
    }
}
