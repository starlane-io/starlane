use std::env::VarError;
use std::str::FromStr;
use once_cell::sync::Lazy;
use std::string::ToString;
use uuid::Uuid;

pub static STARLANE_CONTROL_PORT: Lazy<u16> = Lazy::new(|| {
    std::env::var("STARLANE_PORT")
        .unwrap_or("4343".to_string())
        .parse::<u16>()
        .unwrap_or(4343)
});

pub static STARLANE_HOME: Lazy<String> = Lazy::new(|| {
    std::env::var("STARLANE_HOME").unwrap_or_else(|e| {
        let home_dir: String = match dirs::home_dir() {
            None => ".".to_string(),
            Some(dir) => dir.display().to_string(),
        };
        format!("{}/.starlane", home_dir).to_string()
    })
});
pub static STARLANE_LOG_DIR: Lazy<String> = Lazy::new(|| {
    std::env::var("STARLANE_LOG_DIR").unwrap_or(format!("{}/log", STARLANE_HOME.as_str()).to_string())
});

pub static STARLANE_DATA_DIR: Lazy<String> = Lazy::new(|| {
    std::env::var("STARLANE_DATA_DIR").unwrap_or(format!("{}/data", STARLANE_HOME.as_str()).to_string())
});

pub static STARLANE_CACHE_DIR: Lazy<String> = Lazy::new(|| {
    std::env::var("STARLANE_CACHE_DIR").unwrap_or(format!("{}/cache", STARLANE_HOME.to_string()).to_string())
});

pub static STARLANE_WRITE_LOGS : Lazy<StarlaneWriteLogs> = Lazy::new(|| {
    match std::env::var("STARLANE_WRITE_LOGS")
    {
        Ok(value) => StarlaneWriteLogs::from_str(value.as_str()).unwrap_or_default(),
        Err(err) => StarlaneWriteLogs::default()
    }

});

#[derive(Debug, Clone,strum_macros::EnumString,strum_macros::Display)]
pub enum StarlaneWriteLogs {
    #[strum(serialize = "auto")]
    Auto,
    #[strum(serialize = "file")]
    File,
    #[strum(serialize = "stdout")]
    StdOut
}

impl Default for StarlaneWriteLogs {
    fn default() -> Self {
        Self::Auto
    }
}


static STARLANE_TOKEN: Lazy<String> =
    Lazy::new(|| std::env::var("STARLANE_TOKEN").unwrap_or(Uuid::new_v4().to_string()));
#[cfg(feature = "postgres")]
pub static STARLANE_REGISTRY_URL: Lazy<String> =
    Lazy::new(|| std::env::var("STARLANE_REGISTRY_URL").unwrap_or("localhost".to_string()));
#[cfg(feature = "postgres")]
pub static STARLANE_REGISTRY_USER: Lazy<String> =
    Lazy::new(|| std::env::var("STARLANE_REGISTRY_USER").unwrap_or("postgres".to_string()));
#[cfg(feature = "postgres")]
pub static STARLANE_REGISTRY_PASSWORD: Lazy<String> =
    Lazy::new(|| std::env::var("STARLANE_REGISTRY_PASSWORD").unwrap_or("password".to_string()));
#[cfg(feature = "postgres")]
pub static STARLANE_REGISTRY_DATABASE: Lazy<String> =
    Lazy::new(|| std::env::var("STARLANE_REGISTRY_DATABASE").unwrap_or("postgres".to_string()));

#[cfg(feature = "postgres")]
pub static STARLANE_REGISTRY_SCHEMA: Lazy<String> =
    Lazy::new(|| std::env::var("STARLANE_REGISTRY_SCHEMA").unwrap_or("public".to_string()));
