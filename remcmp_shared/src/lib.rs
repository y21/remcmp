use anyhow::Context;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde::Serialize;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::process::Command;
use tracing::debug;

#[derive(Deserialize, Serialize, Debug)]
pub struct ConnectPacket {
    pub auth: Option<String>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct BuildJob {
    /// The diff as per `git diff`
    pub diff: Vec<u8>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct BuildResponse {
    /// Checksum of the binary
    pub checksum: u32,
    pub binary: Vec<u8>,
}

#[derive(Deserialize, Serialize, Debug)]
pub enum JobKind {
    Build(BuildJob),
    BuildResponse(BuildResponse),
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Job {
    pub kind: JobKind,
}

async fn write_bytes(socket: &mut TcpStream, data: &[u8]) -> anyhow::Result<()> {
    let len: u32 = data
        .len()
        .try_into()
        .with_context(|| format!("Too much data to write: {}b ", data.len()))?;

    socket.write_u32_le(len).await?;
    socket.write_all(data).await?;

    Ok(())
}

async fn read_bytes(socket: &mut TcpStream) -> anyhow::Result<Vec<u8>> {
    let len: u32 = socket.read_u32_le().await?;
    let mut data = vec![0; len as usize];
    socket.read_exact(&mut data).await?;
    Ok(data)
}

async fn parse<T: DeserializeOwned>(socket: &mut TcpStream) -> anyhow::Result<T> {
    let b = read_bytes(socket).await?;
    Ok(bincode::deserialize(&b)?)
}

async fn send<T: Serialize>(socket: &mut TcpStream, data: T) -> anyhow::Result<T> {
    let b = bincode::serialize(&data)?;
    write_bytes(socket, &b).await?;
    Ok(data)
}

pub async fn run_simple_command(cmd: &str, args: &[&str]) -> anyhow::Result<()> {
    let out = Command::new(cmd)
        .args(args)
        .output()
        .await
        .with_context(|| format!("Failed to run {cmd}"))?;

    debug!(%out.status, "{} command finished", cmd);
    Ok(())
}

pub struct Connection {
    inner: TcpStream,
}

impl From<TcpStream> for Connection {
    fn from(inner: TcpStream) -> Self {
        Self { inner }
    }
}

impl Connection {
    pub async fn send_build_job(&mut self, job: BuildJob) -> anyhow::Result<()> {
        send(
            &mut self.inner,
            Job {
                kind: JobKind::Build(job),
            },
        )
        .await?;
        Ok(())
    }

    pub async fn send_connect_packet(&mut self, c: ConnectPacket) -> anyhow::Result<()> {
        send(&mut self.inner, c).await?;
        Ok(())
    }

    pub async fn send_build_response(&mut self, r: BuildResponse) -> anyhow::Result<()> {
        send(
            &mut self.inner,
            Job {
                kind: JobKind::BuildResponse(r),
            },
        )
        .await?;
        Ok(())
    }

    pub async fn recv_job(&mut self) -> anyhow::Result<Job> {
        parse(&mut self.inner).await
    }

    pub async fn recv_connect_packet(&mut self) -> anyhow::Result<ConnectPacket> {
        parse(&mut self.inner).await
    }
}
