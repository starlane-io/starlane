use std::fs;
use std::fs::File;
use chrono::Utc;
use starlane::space::loc::ToBaseKind;
use starlane::space::wasm::Timestamp;
use std::str::FromStr;
use std::sync::Arc;
use atty::Stream;
use colored::Colorize;
use uuid::Uuid;
use starlane::space::err::SpaceErr;
use starlane::space::log::{FileAppender, LogAppender, RootLogger, StdOutAppender};
use crate::env::STARLANE_LOG_DIR;

pub mod err;
pub mod global;
pub mod layer;
pub mod machine;
pub mod reg;
pub mod star;

#[cfg(not(feature="postgres"))]
pub mod tests;

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
    if atty::is(Stream::Stdout) {
/*        fs::create_dir_all(STARLANE_LOG_DIR.to_string())?;
        let writer = BasicRollingFileAppender::new(
            STARLANE_LOG_DIR.to_string(),
            RollingConditionBasic::new().daily(),
            9
        ).unwrap();
        Ok(Arc::new(FileAppender::new( writer )))
 */
            fs::create_dir_all(STARLANE_LOG_DIR.to_string())?;
            let writer = File::create(format!("{}/stdout.log",STARLANE_LOG_DIR.to_string()))?;
            Ok(Arc::new(FileAppender::new( writer )))
        } else {

            Ok(Arc::new(StdOutAppender()))
        }

    }
