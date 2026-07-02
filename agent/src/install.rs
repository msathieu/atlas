use std::{
    env::{self, current_exe},
    error::Error,
    fs::Permissions,
    os::unix::fs::PermissionsExt,
    path::Path,
};
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
    process::Command,
};

pub async fn install_agent() -> Result<(), Box<dyn Error>> {
    if env::consts::OS == "linux" {
        let agent = fs::read(current_exe()?).await?;
        let mut destination = File::create("/usr/local/bin/atlas-agent").await?;
        destination.set_permissions(Permissions::from_mode(0o700)).await?;
        destination.write_all(&agent).await?;
        destination.flush().await?;
        if Path::new("/etc/systemd/system").exists() {
            let mut unit = File::create("/etc/systemd/system/atlas-agent.service").await?;
            unit.write_all(
                br#"[Unit]
After=network-online.target
Wants=network-online.target
[Service]
ExecStart=/usr/local/bin/atlas-agent
Restart=always
PrivateTmp=true
ProtectSystem=true
ReadWritePaths=/usr/local
ProtectHome=true
[Install]
WantedBy=multi-user.target
"#,
            )
            .await?;
            unit.flush().await?;
            Command::new("systemctl").arg("daemon-reload").spawn()?.wait().await?;
            Command::new("systemctl").arg("enable").arg("atlas-agent").spawn()?.wait().await?;
            Command::new("systemctl").arg("restart").arg("atlas-agent").spawn()?.wait().await?;
        }
        return Ok(());
    }
    println!("Unsupported platform!");
    Ok(())
}
