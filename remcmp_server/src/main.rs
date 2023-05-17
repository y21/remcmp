use std::env;
use std::io;
use std::net::SocketAddr;
use std::process::Stdio;
use std::str::FromStr;

use anyhow::bail;
use anyhow::Context;
use remcmp_shared::BuildJob;
use remcmp_shared::BuildResponse;
use remcmp_shared::ConnectPacket;
use remcmp_shared::Job;
use remcmp_shared::JobKind;
use tokio::fs;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::process::Command;
use tracing::debug;
use tracing::error;
use tracing::info;

struct Config {
    compile_cmd: String,
    output_path: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let addr = env::var("REMCMP_ADDR").context("Missing `REMCMP_ADDR` env variable")?;
    let addr = SocketAddr::from_str(&addr).context("Failed to parse `REMCMP_ADDR` env variable")?;
    let auth = env::var("REMCMP_AUTH").ok();
    let compile_cmd = env::var("REMCMP_COMPILE_CMD").unwrap_or_else(|_| "cargo b".into());
    let output_path =
        env::var("REMCMP_OUTPUT_BIN").context("Failed to read `REMCMP_OUTPUT_BIN` env variable")?;

    debug!(?auth);
    if auth.is_none() {
        info!("Starting server in authless mode. Consider setting the `REMCMP_AUTH` environment variable if \
        you are in a public network!");
    }
    info!("Compile command: {}", compile_cmd);

    // Leaking is ok. This happens once and we'll keep the config around forever *anyway*.
    // So there is nothing to be gained by not leaking (though it may trip up memory leak detectors)
    let config: &'static Config = Box::leak(Box::new(Config {
        compile_cmd,
        output_path,
    }));

    let server = TcpListener::bind(addr)
        .await
        .context("Failed to bind TcpListener")?;

    loop {
        let (socket, addr) = server.accept().await?;
        debug!("Accepted connection from {}", addr);
        tokio::spawn(async move {
            if let Err(err) = handle_connection(socket, addr, config).await {
                error!("Error in connection handler: {:?}", err);
            }
        });
    }
}

async fn handle_connection(
    mut socket: TcpStream,
    addr: SocketAddr,
    config: &Config,
) -> anyhow::Result<()> {
    debug!("Awaiting connect packet from {}", addr);
    let packet = remcmp_shared::parse::<ConnectPacket>(&mut socket).await?;

    // TODO: check that packet auth is correct

    let job = remcmp_shared::parse::<Job>(&mut socket).await?;

    match job.kind {
        JobKind::Build(BuildJob { diff }) => {
            info!("Processing build job");

            let c = Command::new("git")
                .args(["checkout", "--", "."])
                .output()
                .await
                .context("Failed to spawn git checkout")?;

            debug!(?c, "git checkout finished");

            let mut cmd = Command::new("git")
                .args(["apply", "-"])
                .stdin(Stdio::piped())
                .spawn()
                .context("Failed to spawn git apply")?;

            cmd.stdin
                .as_mut()
                .unwrap()
                .write_all(diff.as_bytes())
                .await
                .context("Failed to write bytes")?;

            let out = cmd
                .wait_with_output()
                .await
                .context("Failed to run git apply")?;

            debug!(?out, "git apply finished");

            let out = Command::new("sh")
                .arg("-c")
                .arg(&config.compile_cmd)
                .stdout(Stdio::inherit())
                .output()
                .await
                .context("Compile command failed")?;

            debug!(?out, "Compile command finished");

            let binary = fs::read(&config.output_path)
                .await
                .context("Failed to read output path")?;

            let checksum = crc32fast::hash(&binary);
            remcmp_shared::send(
                &mut socket,
                Job {
                    kind: JobKind::BuildResponse(BuildResponse { checksum, binary }),
                },
            )
            .await
            .context("Failed to respond with data")?;
        }
        JobKind::BuildResponse(_) => bail!("Got a message that only the server should send"),
    }

    Ok(())
    // let packet_len = socket.read_u32().await?;
}
// REMCMP_ADDR=127.0.0.1:1234 ~/remcmp/target/debug/remcmp_server
// REMCMP_HOST=127.0.0.1:1234 ~/remcmp/target/debug/remcmp_client
