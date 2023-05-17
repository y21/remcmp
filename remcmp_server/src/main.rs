use std::env;
use std::net::SocketAddr;
use std::process::Stdio;
use std::str::FromStr;

use anyhow::bail;
use anyhow::ensure;
use anyhow::Context;
use remcmp_shared::run_simple_command;
use remcmp_shared::BuildJob;
use remcmp_shared::BuildResponse;
use remcmp_shared::Connection;
use remcmp_shared::JobKind;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::process::Command;
use tracing::debug;
use tracing::error;
use tracing::info;

use crate::config::Config;

mod config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let addr = env::var("REMCMP_ADDR").context("Missing `REMCMP_ADDR` env variable")?;
    let addr = SocketAddr::from_str(&addr).context("Failed to parse `REMCMP_ADDR` env variable")?;

    // Leaking is ok. This happens once and we'll keep the config around forever *anyway*.
    // So there is nothing to be gained by not leaking (though it may trip up memory leak detectors)
    let config: &'static Config = Box::leak(Box::new(Config::create()?));

    debug!(?config);
    if config.auth.is_none() {
        info!("Starting server in authless mode. Consider setting the `REMCMP_AUTH` environment variable if \
        you are in a public network!");
    }

    let server = TcpListener::bind(addr)
        .await
        .context("Failed to bind TcpListener")?;

    loop {
        let (socket, addr) = server.accept().await?;
        debug!("Accepted connection from {}", addr);
        tokio::spawn(async move {
            if let Err(err) = handle_connection(socket.into(), addr, config).await {
                error!("Error in connection handler: {:?}", err);
            }
        });
    }
}

async fn handle_connection(
    mut conn: Connection,
    addr: SocketAddr,
    config: &Config,
) -> anyhow::Result<()> {
    debug!("Awaiting connect packet from {}", addr);
    let packet = conn
        .recv_connect_packet()
        .await
        .context("Failed to receive connect packet")?;

    ensure!(packet.auth == config.auth, "Invalid auth");

    let job = conn.recv_job().await.context("Failed to receive job")?;

    match job.kind {
        JobKind::Build(BuildJob { diff }) => {
            info!("Processing build job");

            run_simple_command("git", &["checkout", "--", "."]).await?;

            let mut cmd = Command::new("git")
                .args(["apply", "-"])
                .stdin(Stdio::piped())
                .spawn()
                .context("Failed to spawn git apply")?;

            cmd.stdin
                .as_mut()
                .unwrap()
                .write_all(&diff)
                .await
                .context("Failed to write bytes")?;

            let out = cmd
                .wait_with_output()
                .await
                .context("Failed to run git apply")?;

            debug!(?out, "git apply finished");

            run_simple_command("sh", &["-c", &config.compile_cmd]).await?;

            debug!(?out, "Compile command finished");

            let binary = fs::read(&config.output_path)
                .await
                .context("Failed to read output path")?;

            let checksum = crc32fast::hash(&binary);
            conn.send_build_response(BuildResponse { checksum, binary })
                .await
                .context("Failed to respond with data")?;
        }
        JobKind::BuildResponse(_) => bail!("Got a message that only the server should send"),
    }

    Ok(())
}
