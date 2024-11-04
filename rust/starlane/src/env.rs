use crate::err::HypErr;
use crate::shutdown::{panic_shutdown, shutdown};
use crate::StarlaneConfig;
use anyhow::anyhow;
use ascii::AsciiChar::G;
use atty::Stream;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::str::FromStr;
use std::string::ToString;
use std::{fs, process};
use tempdir::TempDir;
use uuid::Uuid;

pub fn context() -> String {
    fs::read_to_string(format!("{}/.context", STARLANE_HOME.as_str()).to_string())
        .unwrap_or("default".to_string())
}

pub fn set_context<S>(context: S) -> Result<(), anyhow::Error>
where
    S: AsRef<str>,
{
    fs::create_dir_all(STARLANE_HOME.as_str())?;
    fs::write(
        format!("{}/.context", STARLANE_HOME.as_str()).to_string(),
        context.as_ref().to_string(),
    )?;
    fs::create_dir_all(context_dir()).unwrap();
    Ok(())
}

pub fn context_dir() -> String {
    format!("{}/{}", STARLANE_HOME.as_str(), context()).to_string()
}

pub static STARLANE_CONFIG: Lazy<StarlaneConfig> = Lazy::new(|| match config() {
    Ok(Some(config)) => config,
    Ok(None) => StarlaneConfig::default(),
    Err(err) => {
        eprintln!();
        panic_shutdown(format!(
            "missing or corrupted config file: '{}' ... with error: '{}'",
            config_path(),
            err.to_string()
        ));
        panic!(
            "missing or corrupted config file: '{}' ... with error: '{}'",
            config_path(),
            err.to_string()
        );
    }
});

pub static STARLANE_CONTROL_PORT: Lazy<u16> = Lazy::new(|| {
    std::env::var("STARLANE_PORT")
        .unwrap_or("4343".to_string())
        .parse::<u16>()
        .unwrap_or(4343)
});

#[cfg(not(test))]
pub static STARLANE_HOME: Lazy<String> = Lazy::new(|| {
    std::env::var("STARLANE_HOME").unwrap_or_else(|e| {
        let home_dir: String = match dirs::home_dir() {
            None => ".".to_string(),
            Some(dir) => dir.display().to_string(),
        };
        format!("{}/.starlane", home_dir).to_string()
    })
});

#[cfg(test)]
pub static STARLANE_HOME: Lazy<String> = Lazy::new(|| {
    let dir = ".starlane_test";
    fs::create_dir_all(dir).unwrap();
    dir.to_string()
});

pub static STARLANE_GLOBAL_SETTINGS: Lazy<GlobalSettings> = Lazy::new(|| ensure_global_settings());

pub static STARLANE_LOG_DIR: Lazy<String> = Lazy::new(|| {
    std::env::var("STARLANE_LOG_DIR")
        .unwrap_or(format!("{}/log", STARLANE_HOME.as_str()).to_string())
});

pub static STARLANE_DATA_DIR: Lazy<String> = Lazy::new(|| {
    std::env::var("STARLANE_DATA_DIR")
        .unwrap_or(format!("{}/data", STARLANE_HOME.as_str()).to_string())
});

pub static STARLANE_CACHE_DIR: Lazy<String> = Lazy::new(|| {
    std::env::var("STARLANE_CACHE_DIR")
        .unwrap_or(format!("{}/cache", STARLANE_HOME.to_string()).to_string())
});

pub static STARLANE_WRITE_LOGS: Lazy<StarlaneWriteLogs> =
    Lazy::new(|| match std::env::var("STARLANE_WRITE_LOGS") {
        Ok(value) => StarlaneWriteLogs::from_str(value.as_str()).unwrap_or_default(),
        Err(err) => StarlaneWriteLogs::default(),
    });

#[derive(Debug, Clone, strum_macros::EnumString, strum_macros::Display)]
pub enum StarlaneWriteLogs {
    #[strum(serialize = "auto")]
    Auto,
    #[strum(serialize = "file")]
    File,
    #[strum(serialize = "stdout")]
    StdOut,
}

impl Default for StarlaneWriteLogs {
    fn default() -> Self {
        Self::Auto
    }
}

static STARLANE_TOKEN: Lazy<String> =
    Lazy::new(|| std::env::var("STARLANE_TOKEN").unwrap_or(Uuid::new_v4().to_string()));

pub fn config_path() -> String {
    config_path_context(context())
}

pub fn config_path_context(context: String) -> String {
    format!("{}/{}/config.yaml", STARLANE_HOME.as_str(), context).to_string()
}

pub fn config_exists(context: String) -> bool {
    fs::exists(config_path_context(context)).unwrap_or(false)
}

#[cfg(feature = "server")]
pub fn config() -> Result<Option<StarlaneConfig>, HypErr> {
    let file = config_path();

    match fs::exists(file.clone())? {
        true => {
            let config = std::fs::read_to_string(file.clone())?;

            let mut config: StarlaneConfig  = serde_yaml::from_str(config.as_str()).map_err(|err| anyhow!("starlane config found: '{}' yet Starlane encountered an error when attempting to process the config: '{}'", config_path(), err))?;
            config.context = context();

            Ok(Some(config))
        }
        false => Ok(None),
    }
}
pub fn config_save(config: StarlaneConfig) -> Result<(), anyhow::Error> {
    let file = config_path();
    config_save_new(config, file)
}

pub fn config_save_new(config: StarlaneConfig, file: String) -> Result<(), anyhow::Error> {
    match serde_yaml::to_string(&config) {
        Ok(ser) => {
            let file: PathBuf = file.into();
            match file.parent() {
                Some(dir) => {
                    std::fs::create_dir_all(dir)?;
                    std::fs::write(file, ser)?;
                    Ok(())
                }
                None => {
                    Err(anyhow!("starlane encountered an error when attempting to save config file: 'invalid parent'"))
                }
            }
        }
        Err(err) => Err(anyhow!(
            "starlane internal error: 'could not deserialize config"
        )),
    }
}

pub fn global_settings_path() -> String {
    format!("{}/global.conf", STARLANE_HOME.to_string()).to_string()
}

pub fn global_settings_exists() -> bool {
    fs::exists(global_settings_path()).unwrap_or(false)
}

pub fn ensure_global_settings() -> GlobalSettings {
    if !global_settings_exists() {
        save_global_settings(GlobalSettings::default()).unwrap();
    }
    load_global_settings().unwrap()
}

pub fn load_global_settings() -> Result<GlobalSettings, anyhow::Error> {
    let string = fs::read_to_string(global_settings_path())?;
    Ok(serde_yaml::from_str(string.as_str())
        .map_err(|err| {
            eprintln!(
                "{}",
                anyhow!(
                    "could not process global settings: '{}' caused by err '{}'",
                    global_settings_path(),
                    err
                )
                .to_string()
            )
        })
        .unwrap_or(GlobalSettings::default()))
}

pub fn save_global_settings(settings: GlobalSettings) -> Result<(), anyhow::Error> {
    let settings = serde_yaml::to_string(&settings).map_err(|err| {
        anyhow!(
            "could not process global settings: '{}' caused by err '{}'",
            global_settings_path(),
            err
        )
    })?;
    let path: PathBuf = global_settings_path().into();
    fs::create_dir_all(path.parent().unwrap())?;
    fs::write(global_settings_path(), settings)?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSettings {
    pub nuke: bool,
    pub mode: GlobalMode,
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            nuke: false,
            mode: GlobalMode::Newbie,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GlobalMode {
    Newbie,
    Expert,
}

pub trait Enviro {
    fn is_terminal(&self) -> bool;
    fn term_width(&self) -> usize;
}

pub struct StdEnviro();

impl Default for StdEnviro {
    fn default() -> Self {
        StdEnviro()
    }
}

impl Enviro for StdEnviro {
    fn is_terminal(&self) -> bool {
        atty::is(Stream::Stdout)
    }

    fn term_width(&self) -> usize {
        match termsize::get() {
            None => 128,
            Some(size) => size.cols as usize,
        }
    }
}
