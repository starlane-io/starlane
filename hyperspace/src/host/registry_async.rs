use std::future::Future;
use std::str::FromStr;
use anyhow::Result;
use wasmtime::{
    Config,
    Engine,
    Store,
};
use wasmtime::component::{Component, HasSelf, Linker, ResourceTable};
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};
use wasmtime_wasi::p2::bindings::Command;
use starlane_space::types::specific::PackFile;
use bindings::starlane::hyperspace::space::Status;

mod bindings {
    wasmtime::component::bindgen!({
            path: "../wit",
            world: "registry",

    imports: { default: async | trappable },
    exports: { default: async },
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
use bindings::exports::starlane::hyperspace::registry_api::RegErr;
use bindings::Registry;

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
        async fn update(&mut self, status: bindings::starlane::hyperspace::status_api::Status) -> wasmtime::Result<()>{
            todo!()
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
async fn test_mock_registry_async_main() -> Result<()> {
    let mut config = Config::default();
    config.wasm_component_model(true);
    /*
    config.wasm_component_model_async(true);
    config.wasm_component_model_async_stackful(true);
    config.wasm_component_model_more_async_builtins(true);
    config.wasm_component_model_threading(true);
    
     */

    let engine = Engine::new(&config)?;

    let file = PackFile::from_str("starlane.app:examples:0.1.0/mock_registry_async.wasm")?;
    let cache = starlane_package::cache::cache_singleton();
    let path = cache.get_path(&file).await.unwrap();
    let component = Component::from_file(&engine, path)?;
    let mut wasi = WasiCtx::builder();
    wasi.inherit_stdout().inherit_stdin().inherit_stderr();

    let ctx = wasi.build();
    let state = RegistryState::default();

    let mut linker = Linker::new(&engine);
//    RegistryComponent::add_to_linker::<_,HasSelf<_>>(&mut linker, |state: &mut RegistryState| state)?;
    wasmtime_wasi::p2::add_to_linker_sync(&mut linker).unwrap();

    let mut store = Store::new(&engine, state);

    let guest = Command::instantiate_async(&mut store, &component, &linker).await?;
    guest.wasi_cli_run().call_run(&mut store).await?;

    Ok(())
}

#[tokio::test]
async fn test_mock_registry_async() -> Result<()> {
    let mut config = Config::default();
    config.wasm_component_model(true);
    config.wasm_component_model_async(true);
    config.wasm_component_model_async_stackful(true);
    config.wasm_component_model_more_async_builtins(true);
    config.wasm_component_model_threading(true);
    config.async_support(true);

    let engine = Engine::new(&config)?;

    let file = PackFile::from_str("starlane.app:examples:0.1.0/mock_registry_async.wasm")?;
    let cache = starlane_package::cache::cache_singleton();
    let path = cache.get_path(&file).await.unwrap();
    let component = Component::from_file(&engine, path)?;
    let mut wasi = WasiCtx::builder();
    wasi.inherit_stdout().inherit_stdin().inherit_stderr();

    let ctx = wasi.build();
    let state = RegistryState::default();

    let mut linker = Linker::new(&engine);
    RegistryComponent::add_to_linker::<_,HasSelf<_>>(&mut linker, |state: &mut RegistryState| state)?;
    wasmtime_wasi::p2::add_to_linker_sync(&mut linker).unwrap();

    let mut store = Store::new(&engine, state);

    let bindings = RegistryComponent::instantiate(&mut store, &component, &linker)?;
    let guest = bindings.starlane_hyperspace_registry_api();



    match guest.call_scorch(& mut store).await.unwrap() {
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