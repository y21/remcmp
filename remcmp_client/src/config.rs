use std::env;
use std::net::SocketAddr;

use anyhow::Context;

pub struct Config {
    pub host: SocketAddr,
    pub output_path: String,
    pub auth: Option<String>,
}

impl Config {
    pub fn create() -> anyhow::Result<Self> {
        let host: SocketAddr = env::var("REMCMP_HOST")
            .map(|h| h.parse())
            .context("Failed to read `REMCMP_HOST` env variable")?
            .context("Failed to parse `REMCMP_HOST` env variable")?;

        let output_path =
            env::var("REMCMP_OUTPUT_BIN").context("Missing `REMCMP_OUTPUT_BIN` env variable")?;

        let auth = env::var("REMCMP_AUTH").ok();

        Ok(Self {
            auth,
            host,
            output_path,
        })
    }
}
