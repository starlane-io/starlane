use once_cell::sync::Lazy;
use uuid::Uuid;

pub static STARLANE_CONTROL_PORT: Lazy<u16> = Lazy::new(|| {
    std::env::var("STARLANE_PORT")
        .unwrap_or("4343".to_string())
        .parse::<u16>()
        .unwrap_or(4343)
});
pub static STARLANE_DATA_DIR: Lazy<String> =
    Lazy::new(|| std::env::var("STARLANE_DATA_DIR").unwrap_or("./data/".to_string()));
static STARLANE_CACHE_DIR: Lazy<String> =
    Lazy::new(|| std::env::var("STARLANE_CACHE_DIR").unwrap_or("cache".to_string()));
static STARLANE_TOKEN: Lazy<String> =
    Lazy::new(|| std::env::var("STARLANE_TOKEN").unwrap_or(Uuid::new_v4().to_string()));
pub static STARLANE_REGISTRY_URL: Lazy<String> =
    Lazy::new(|| std::env::var("STARLANE_REGISTRY_URL").unwrap_or("localhost".to_string()));
pub static STARLANE_REGISTRY_USER: Lazy<String> =
    Lazy::new(|| std::env::var("STARLANE_REGISTRY_USER").unwrap_or("postgres".to_string()));
pub static STARLANE_REGISTRY_PASSWORD: Lazy<String> =
    Lazy::new(|| std::env::var("STARLANE_REGISTRY_PASSWORD").unwrap_or("password".to_string()));
pub static STARLANE_REGISTRY_DATABASE: Lazy<String> =
    Lazy::new(|| std::env::var("STARLANE_REGISTRY_DATABASE").unwrap_or("postgres".to_string()));
