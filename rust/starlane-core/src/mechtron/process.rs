use mesh_portal_serde::version::latest::id::Address;
use std::process::{Child, Command};
use crate::error::Error;
use crate::star::core::resource::manager::mechtron::STARLANE_MECHTRON_PORT;

pub fn launch_mechtron_process(wasm_src: Address ) -> Result<Child,Error> {
    let host = format!("localhost:{}",STARLANE_MECHTRON_PORT);
    let child = Command::new("starlane")
        .arg("mechtron")
        .arg(host.as_str())
        .arg(wasm_src.to_string().as_str()).spawn()?;
    Ok(child)
}