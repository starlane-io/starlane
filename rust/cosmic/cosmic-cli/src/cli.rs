use dirs;
use serde::{Deserialize, Serialize};
use std::fs::{DirBuilder, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Mutex;
use crate::err::CliErr;



lazy_static! {
    pub static ref CLI_CONFIG: Mutex<CliConfig> = Mutex::new(CliConfig::load_or_default());
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CliConfig {
    pub hostname: String,
    pub oauth_url: Option<String>,
    pub refresh_token: Option<String>
}

impl CliConfig {
    pub fn default() -> Self {
        Self {
            hostname: "localhost:4343".to_string(),
            refresh_token: None,
            oauth_url: None
        }
    }

    pub fn load_or_default() -> Self {
        match Self::load() {
            Ok(cli_config) => cli_config,
            Err(_err) => Self::default(),
        }
    }

    pub fn load() -> Result<Self, CliErr> {
        let root = match dirs::home_dir() {
            None => PathBuf::from_str("./")?,
            Some(path) => path,
        };
        let dir = format!("{}/.cosmic", root.to_str().unwrap_or(".").to_string());

        let path = format!("{}/cli.json", dir);

        let mut file = File::open(path)?;

        let mut buf = vec![];
        file.read_to_end(&mut buf)?;
        let cli_config = serde_json::from_str(String::from_utf8(buf)?.as_str())?;
        Ok(cli_config)
    }

    pub fn save(&self) -> Result<(), CliErr> {
        let root = match dirs::home_dir() {
            None => PathBuf::from_str("./")?,
            Some(path) => path,
        };
        let dir = format!(
            "{}/.cosmic",
            root.to_str()
                .ok_or("CliConfig: expected HOME dir path")?
                .to_string()
        );

        let mut builder = DirBuilder::new();
        builder.recursive(true);
        builder.create(dir.clone())?;

        let path = format!("{}/cli.json", dir);

        let mut file = File::create(path)?;

        let json = serde_json::to_string(self)?;

        file.write_all(json.as_bytes())?;

        Ok(())
    }
}

