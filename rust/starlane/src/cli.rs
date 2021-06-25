use std::sync::Mutex;
use starlane_core::error::Error;
use std::fs::{File, DirBuilder};
use dirs;
use std::path::PathBuf;
use serde::{Serialize,Deserialize};
use std::str::FromStr;
use std::io::{Write, Read};

lazy_static!{

    pub static ref CLI_CONFIG: Mutex<CliConfig> = Mutex::new( CliConfig::load_or_default() );

}

#[derive(Clone,Serialize,Deserialize)]
pub struct CliConfig{
    pub hostname: String,
}

impl CliConfig{
    pub fn default() -> Self {
        Self{
            hostname: format!("localhost:{}", starlane_core::starlane::DEFAULT_PORT.clone() )
        }
    }
    pub fn load_or_default( ) -> Self {
        match Self::load() {
            Ok(cli_config) => {
                cli_config
            }
            Err(err) => {
                Self::default()
            }
        }
    }

    pub fn load( ) -> Result<Self,Error> {

        let root = match dirs::home_dir() {
            None => {
                PathBuf::from_str("./")?
            }
            Some(path) => path
        };
        let dir = format!( "{}/.starlane", root.to_str().unwrap_or(".").to_string() );

        let path = format!("{}/cli.json",dir);

        let mut file = File::open(path)?;

        let mut buf = vec![];
        file.read_to_end(& mut buf )?;
        let cli_config = serde_json::from_str(String::from_utf8(buf)?.as_str() )?;
        Ok(cli_config)
    }

    pub fn save( &self ) -> Result<(),Error> {

        let root = match dirs::home_dir() {
            None => {
                PathBuf::from_str("./")?
            }
            Some(path) => path
        };
        let dir = format!( "{}/.starlane", root.to_str().ok_or("CliConfig: expected HOME dir path")?.to_string() );

        let mut builder = DirBuilder::new();
        builder.recursive(true);
        builder.create(dir.clone())?;

        let path = format!("{}/cli.json",dir);

        let mut file = File::create(path)?;

        let json = serde_json::to_string(self)?;

        file.write_all(json.as_bytes())?;

        Ok(())
    }
}