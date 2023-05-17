use anyhow::Context;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde::Serialize;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

#[derive(Deserialize, Serialize, Debug)]
pub struct ConnectPacket {
    pub auth: Option<String>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct BuildJob {
    /// The diff as per `git diff`
    pub diff: String,
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

pub async fn write_bytes(socket: &mut TcpStream, data: &[u8]) -> anyhow::Result<()> {
    let len: u32 = data
        .len()
        .try_into()
        .with_context(|| format!("Too much data to write: {}b ", data.len()))?;

    socket.write_u32_le(len).await?;
    socket.write_all(data).await?;

    Ok(())
}

pub async fn read_bytes(socket: &mut TcpStream) -> anyhow::Result<Vec<u8>> {
    let len: u32 = socket.read_u32_le().await?;
    let mut data = vec![0; len as usize];
    socket.read_exact(&mut data).await?;
    Ok(data)
}

pub async fn parse<T: DeserializeOwned>(socket: &mut TcpStream) -> anyhow::Result<T> {
    let b = read_bytes(socket).await?;
    Ok(bincode::deserialize(&b)?)
}

pub async fn send<T: Serialize>(socket: &mut TcpStream, data: T) -> anyhow::Result<T> {
    let b = bincode::serialize(&data)?;
    write_bytes(socket, &b).await?;
    Ok(data)
}
