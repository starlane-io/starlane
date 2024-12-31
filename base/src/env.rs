use starlane_hyperspace::err::HypErr;
use starlane_hyperspace::shutdown::panic_shutdown;

use anyhow::anyhow;
use atty::Stream;
use chrono::Utc;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::env::current_dir;
use std::fs;
use std::fs::File;
use std::ops::Deref;
use std::path::PathBuf;
use std::str::FromStr;
use std::string::ToString;
use std::sync::Arc;
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

#[cfg(feature = "server")]
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
        format!("{}/.main", home_dir).to_string()
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
    std::env::var("STARLANE_DATA_DIR").unwrap_or_else(|e| {
        let dir: String = match dirs::home_dir() {
            None => current_dir().unwrap().display().to_string(),
            Some(dir) => dir.display().to_string(),
        };
        format!("{}/main/data", dir).to_string()
    })
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

            let mut config: StarlaneConfig = serde_yaml::from_str(config.as_str()).map_err(|err| anyhow!("main config found: '{}' yet Starlane encountered an error when attempting to process the config: '{}'", config_path(), err))?;
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
                    Err(anyhow!("main encountered an error when attempting to save config file: 'invalid parent'"))
                }
            }
        }
        Err(err) => Err(anyhow!(
            "main internal error: 'could not deserialize config"
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

/*
#[no_mangle]
pub extern "C" fn starlane_uuid() -> String {
    Uuid::new_v4().to_string()
}

#[no_mangle]
pub extern "C" fn starlane_timestamp() -> Timestamp {
    Timestamp::new(Utc::now().timestamp_millis())
}

 */

/*
#[no_mangle]
extern "C" fn starlane_root_log_appender() -> Result<Arc<dyn LogAppender>, SpaceErr> {
    let append_to_file = match &STARLANE_WRITE_LOGS.deref() {
        StarlaneWriteLogs::Auto => atty::is(Stream::Stdout),
        StarlaneWriteLogs::File => true,
        StarlaneWriteLogs::StdOut => false,
    };

    if append_to_file {
        fs::create_dir_all(STARLANE_LOG_DIR.to_string())?;
        let writer = File::create(format!("{}/stdout.log", STARLANE_LOG_DIR.to_string()))?;
        Ok(Arc::new(FileAppender::new(writer)))
    } else {
        Ok(Arc::new(StdOutAppender()))
    }
}

 */
