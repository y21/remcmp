use std::process::Output;

use anyhow::bail;
use anyhow::ensure;
use anyhow::Context;
use config::Config;
use remcmp_shared::BuildJob;
use remcmp_shared::BuildResponse;
use remcmp_shared::ConnectPacket;
use remcmp_shared::Connection;
use remcmp_shared::JobKind;
use tokio::fs;
use tokio::net::TcpStream;
use tokio::process::Command;
use tracing::debug;
use tracing::info;

use crate::util::matches_checksum;

mod config;
mod util;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config = Config::create().context("Failed to create config")?;

    let (mut conn, diff) = tokio::try_join!(connect(&config), git_diff())?;

    debug!("Sending diff");

    conn.send_build_job(BuildJob { diff })
        .await
        .context("Failed to send diff packet")?;

    let response = conn
        .recv_job()
        .await
        .context("Failed to receive response")?;

    match response.kind {
        JobKind::BuildResponse(BuildResponse { checksum, binary }) => {
            info!("Received build response");
            ensure!(matches_checksum(checksum, &binary), "Checksum mismatch!");

            info!("Writing binary to {}", config.output_path);
            fs::write(&config.output_path, binary).await?;

            Command::new("chmod")
                .args(["+x", &config.output_path])
                .output()
                .await
                .context("Failed to run chmod")?;
        }
        JobKind::Build(_) => bail!("Only the client sends this"),
    }

    Ok(())
}

async fn connect(conf: &Config) -> anyhow::Result<Connection> {
    let mut conn: Connection = TcpStream::connect(conf.host)
        .await
        .context("Failed to connect to host")?
        .into();

    info!("Connected to host. Sending connect packet");

    conn.send_connect_packet(ConnectPacket {
        auth: conf.auth.clone(),
    })
    .await
    .context("Failed to send connect packet")?;

    Ok(conn)
}

async fn git_diff() -> anyhow::Result<Vec<u8>> {
    let Output { status, stdout, .. } = Command::new("git")
        .arg("diff")
        .output()
        .await
        .context("Failed to run `git diff`")?;

    debug!(%status);
    Ok(stdout)
}
