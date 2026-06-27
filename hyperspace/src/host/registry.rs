use std::str::FromStr;
use anyhow::Result;
use wasmtime::{
    Config,
    Engine,
    Store,
};
use wasmtime::component::{Component, HasSelf, Linker, ResourceTable};
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};
use starlane_space::types::specific::PackFile;
use bindings::starlane::hyperspace::space::Status;

mod bindings {
    wasmtime::component::bindgen!({
            path: "../wit",
            world: "registry",
        });
}

pub use bindings::Registry as RegistryComponent;

pub mod wit {
    pub use super::bindings::starlane::hyperspace::space;
    pub use super::bindings::starlane::hyperspace::status_api::Host as StatusHost;
    pub use super::bindings::starlane::hyperspace::space::Host as SpaceHost;
    pub use super::bindings::exports::starlane::hyperspace::registry_api::Guest as RegistryGuest;
}

pub use bindings::starlane::hyperspace::space;
pub use bindings::starlane::hyperspace::status_api::Host as StatusHost;
use crate::host::registry::bindings::exports::starlane::hyperspace::registry_api::RegErr;

struct RegistryState {
    pub status: Status,
    ctx: WasiCtx,
    table: ResourceTable
}

impl RegistryState {
    pub fn status(&self) -> &'static str {
        match self.status {
            Status::Unknown => "unknown",
            Status::Pending => "pending",
            Status::Init => "init",
            Status::Panic => "panic",
            Status::Fatal => "fatal",
            Status::Ready => "ready",
            Status::Paused => "paused",
            Status::Resuming => "resuming",
            Status::Done => "done",
        }
    }
}

impl Default for RegistryState {
    fn default() -> Self {
        Self {
            status: Status::Unknown,
            ctx: WasiCtx::default(),
            table: ResourceTable::default(),
        }
    }
}

    impl wit::StatusHost for RegistryState {
        fn update(&mut self, status: bindings::starlane::hyperspace::status_api::Status) -> () {
            self.status = status.into();
        }
    }


impl wit::SpaceHost for RegistryState { }
    impl WasiView for RegistryState {
        fn ctx(&mut self) -> WasiCtxView<'_> {
            WasiCtxView {
                ctx: &mut self.ctx,
                table: &mut self.table,
            }
        }
    }






mod state {
    use starlane_space::particle::Status;
    struct HostState {
        pub status_tx: tokio::sync::watch::Sender<Status>
    }   
}

#[tokio::test]
async fn test_mock_registry() -> Result<()> {
    let engine = Engine::default();

    let file = PackFile::from_str("starlane.app:examples:0.1.0/mock_registry.wasm")?;
    let cache = starlane_package::cache::cache_singleton();
    let path = cache.get_path(&file).await.unwrap();
    let component = Component::from_file(&engine, path)?;
    let mut wasi = WasiCtx::builder();

    let ctx = wasi.build();
    let state = RegistryState::default();

    let mut linker = Linker::new(&engine);
    RegistryComponent::add_to_linker::<_,HasSelf<_>>(&mut linker, |state: &mut RegistryState| state)?;
    wasmtime_wasi::p2::add_to_linker_sync(&mut linker).unwrap();

    let mut store = Store::new(&engine, state);

    println!("Blah");

    let bindings = RegistryComponent::instantiate(&mut store, &component, &linker)?;
    let guest = bindings.starlane_hyperspace_registry_api();
     match guest.call_assign_host(& mut store, & "hello".to_string(), &"kitty".to_string()).unwrap() {
         Ok(_) => {
             assert!(false)
         }
         Err(err) => {
             match err {
                 RegErr::NotImplemented => {

                 },
                 _ => {
                     assert!(false)
                 }
             }
         }
     }

    match guest.call_scorch(& mut store).unwrap() {
        Ok(_) => {
            assert!(true)
        }
        Err(err) => {
            assert!(false)
       }
    }

    Ok(())
}




    

/*


fn main() -> Result<()> {
    let mut config = Config::new();
    config.wasm_component_model(true);

    let engine = Engine::new(&config)?;

    let component =
        Component::from_file(&engine, "guest_component.wasm")?;

    let mut linker = Linker::<RegistryState>::new(&engine);

    bindings::example::calculator::calculator::add_to_linker(
        &mut linker,
        |state| state,
    )?;

    let mut store = Store::new(&engine, RegistryState);

    let (_instance, _) =
        bindings::App::instantiate(
            &mut store,
            &component,
            &linker,
        )?;

    Ok(())
}

 */