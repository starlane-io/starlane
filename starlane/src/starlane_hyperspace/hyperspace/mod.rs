use std::fs;
use std::fs::File;
use std::ops::Deref;
use chrono::Utc;
use starlane::space::loc::ToBaseKind;
use starlane::space::wasm::Timestamp;
use std::str::FromStr;
use std::sync::Arc;
use atty::Stream;
use colored::Colorize;
use uuid::Uuid;
use starlane::space::err::SpaceErr;
use starlane::space::log::{FileAppender, LogAppender, StdOutAppender};
use crate::starlane_hyperspace::hyperspace::env::{StarlaneWriteLogs, STARLANE_LOG_DIR, STARLANE_WRITE_LOGS};

pub mod layer;
pub mod err;
pub mod global;
pub mod machine;
pub mod reg;
pub mod star;

#[cfg(not(feature="postgres"))]
pub mod tests;

#[cfg(not(feature="postgres"))]
pub mod tests;
pub mod driver;
#[cfg(feature = "hyperlane")]
pub mod hyperlane;
pub mod registry;
pub mod executor;
pub mod host;
pub mod env;
pub mod shutdown;

#[no_mangle]
pub extern "C" fn starlane_uuid() -> String {
    Uuid::new_v4().to_string()
}

#[no_mangle]
pub extern "C" fn starlane_timestamp() -> Timestamp {
    Timestamp::new(Utc::now().timestamp_millis())
}

#[no_mangle]
extern "C" fn starlane_root_log_appender() -> Result<Arc<dyn LogAppender>,SpaceErr> {

    let append_to_file = match &STARLANE_WRITE_LOGS.deref() {
        StarlaneWriteLogs::Auto => {
            atty::is(Stream::Stdout)
        }
        StarlaneWriteLogs::File => true,
        StarlaneWriteLogs::StdOut => false
    };

    if append_to_file {
            fs::create_dir_all(STARLANE_LOG_DIR.to_string())?;
            let writer = File::create(format!("{}/stdout.log",STARLANE_LOG_DIR.to_string()))?;
            Ok(Arc::new(FileAppender::new( writer )))
        } else {

            Ok(Arc::new(StdOutAppender()))
        }

    }
