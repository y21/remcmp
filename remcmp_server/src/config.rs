use std::env;

use anyhow::Context;

#[derive(Debug)]
pub struct Config {
    pub compile_cmd: String,
    pub output_path: String,
    pub auth: Option<String>,
}

impl Config {
    pub fn create() -> anyhow::Result<Self> {
        let auth = env::var("REMCMP_AUTH").ok();
        let compile_cmd = env::var("REMCMP_COMPILE_CMD").unwrap_or_else(|_| "cargo b".into());
        let output_path = env::var("REMCMP_OUTPUT_BIN")
            .context("Failed to read `REMCMP_OUTPUT_BIN` env variable")?;

        Ok(Self {
            compile_cmd,
            output_path,
            auth,
        })
    }
}
