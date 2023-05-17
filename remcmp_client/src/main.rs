use std::env;
use std::net::SocketAddr;
use std::process::Output;

use anyhow::bail;
use anyhow::Context;
use remcmp_shared::BuildJob;
use remcmp_shared::BuildResponse;
use remcmp_shared::ConnectPacket;
use remcmp_shared::Job;
use remcmp_shared::JobKind;
use tokio::fs;
use tokio::net::TcpStream;
use tokio::process::Command;
use tracing::debug;
use tracing::info;

struct Config {
    host: SocketAddr,
    output_path: String,
    auth: Option<String>,
}

async fn connect(conf: &Config) -> anyhow::Result<TcpStream> {
    let mut conn = TcpStream::connect(conf.host)
        .await
        .context("Failed to connect to host")?;

    info!("Connected to host. Sending connect packet");

    remcmp_shared::send(
        &mut conn,
        ConnectPacket {
            auth: conf.auth.clone(),
        },
    )
    .await
    .context("Failed to send connect packet")?;

    Ok(conn)
}

async fn git_diff() -> anyhow::Result<String> {
    let Output { status, stdout, .. } = Command::new("git")
        .arg("diff")
        .output()
        .await
        .context("Failed to run `git diff`")?;

    debug!(%status);
    String::from_utf8(stdout).context("Invalid UTF8 in diff")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let host: SocketAddr = env::var("REMCMP_HOST")
        .map(|h| h.parse())
        .context("Failed to read `REMCMP_HOST` env variable")?
        .context("Failed to parse `REMCMP_HOST` env variable")?;

    let output_path =
        env::var("REMCMP_OUTPUT_BIN").context("Missing `REMCMP_OUTPUT_BIN` env variable")?;

    let auth = env::var("REMCMP_AUTH").ok();

    debug!(?auth);

    let config = Config {
        auth,
        host,
        output_path,
    };

    let connect_fut = connect(&config);
    let diff_fut = git_diff();

    let (mut conn, diff) = tokio::try_join!(connect_fut, diff_fut)?;

    debug!("Sending diff");

    remcmp_shared::send(
        &mut conn,
        Job {
            kind: JobKind::Build(BuildJob { diff }),
        },
    )
    .await
    .context("Failed to send diff packet")?;

    let response = remcmp_shared::parse::<Job>(&mut conn)
        .await
        .context("Failed to receive response")?;

    match response.kind {
        JobKind::BuildResponse(BuildResponse { checksum, binary }) => {
            info!("Received build response");
            let local_checksum = crc32fast::hash(&binary);
            if checksum != local_checksum {
                bail!(
                    "Checksum mismatch! {} (recv) != {} (local)",
                    checksum,
                    local_checksum
                );
            }

            info!("Checksum matches. Writing binary to {}", config.output_path);
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
