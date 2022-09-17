use cosmic_universe::loc::StarKey;
lazy_static! {
    pub static ref REGISTRY_URL: String =
        std::env::var("REGISTRY_URL").unwrap_or("localhost".to_string());
    pub static ref REGISTRY_USER: String =
        std::env::var("REGISTRY_USER").unwrap_or("postgres".to_string());
    pub static ref REGISTRY_PASSWORD: String =
        std::env::var("REGISTRY_PASSWORD").unwrap_or("password".to_string());
    pub static ref REGISTRY_DATABASE: String =
        std::env::var("REGISTRY_DATABASE").unwrap_or("postgres".to_string());
}

pub struct DBInfo {
    pub url: String,
    pub user: String,
    pub password: String,
    pub database: String,
}

impl DBInfo {
    pub fn to_uri(&self) -> String {
        format!(
            "postgres://{}:{}@{}/{}",
            self.user, self.password, self.url, self.database
        )
    }
}

pub fn lookup_registry_db() -> DBInfo {
    DBInfo {
        url: REGISTRY_URL.to_string(),
        database: REGISTRY_DATABASE.to_string(),
        user: REGISTRY_USER.to_string(),
        password: REGISTRY_PASSWORD.to_string(),
    }
}

pub fn lookup_db_for_star(star_key: &StarKey) -> DBInfo {
    // future versions need a way to lookup this info
    DBInfo {
        url: REGISTRY_URL.to_string(),
        database: REGISTRY_DATABASE.to_string(),
        user: REGISTRY_USER.to_string(),
        password: REGISTRY_PASSWORD.to_string(),
    }
}
