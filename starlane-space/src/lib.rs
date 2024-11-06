#![allow(warnings)]
/*
#![feature(prelude_import)]
#![feature(custom_inner_attributes)]
#![feature(proc_macro_hygiene)]

 */
//#![starlane_primitive_macros::loggerhead]
//extern crate alloc;
#[macro_use]
extern crate async_trait;
#[macro_use]
extern crate enum_ordinalize; //# ! [feature(unboxed_closures)]
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate strum_macros;


extern crate core;

use core::str::FromStr;
use std::ops::Deref;

use serde::{Deserialize, Serialize};

use space::artifact::asynch::ArtifactFetcher;
use space::command::common::SetProperties;
use space::command::direct::create::KindTemplate;
use space::command::direct::delete::Delete;
use space::command::direct::select::Select;
use space::config::bind::BindConfig;
use space::kind::{BaseKind, Kind, StarSub};
use space::loc::Surface;
use space::particle::{Details, Status, Stub};
use space::substance::Bin;
use space::substance::{Substance, ToSubstance};
use space::wave::core::ReflectedCore;

use space::err::SpaceErr;
use space::hyper::ParticleRecord;
use space::wave::Agent;


pub extern crate self as starlane;





pub mod space;


/*
pub fn starlane_uuid() -> Uuid {
    uuid::Uuid::new_v4().to_string()
}
pub fn starlane_timestamp() -> DateTime<Utc> {
    Utc::now()
}

 */

#[cfg(test)]
pub mod tests {
    use crate::space::VERSION;

    #[test]
    fn version() {
        println!("VERSION: {}", VERSION.to_string());
    }
}