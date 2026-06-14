#![allow(warnings)]
#![allow(unused)]

use starlane_package::cache::PackageCache;
use std::process;
use std::str::FromStr;
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    // Define the WASI functions globally on the `Config`.
    let engine = Engine::default();
    let mut linker = wasmtime::component::Linker::new(&engine);
    wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;

    // Create a WASI context and put it in a Store; all instances in the store
    // share this context. `WasiCtx` provides a number of ways to
    // configure what the target program will have access to.
    let wasi = WasiCtx::builder().inherit_stdio().inherit_args().build();
    let state = ComponentRunStates {
        wasi_ctx: wasi,
        resource_table: ResourceTable::new(),
    };
    let mut store = Store::new(&engine, state);

    let file = PackFile::from_str("starlane.app:examples:0.1.0/hello_wasip2.wasm")?;
    let path= wasmtime_wasi::runtime::in_tokio(async move {
        let cache = starlane_package::cache::cache_singleton();
         cache.get_path(&file).await
    })?;


    // Instantiate our component with the imports we've created, and run it.
    let component = Component::from_file(&engine, path).unwrap();
    let command = Command::instantiate(&mut store, &component, &linker)?;
    let program_result = command.wasi_cli_run().call_run(&mut store)?;
    Ok(())
}

use starlane_package::PackFile;
use wasmtime::component::{Component, ResourceTable};
use wasmtime::{Caller, Config, Engine, Linker, Module, Store};
use wasmtime_wasi::p2::bindings::sync::Command;
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

pub struct RunHost {}

pub struct ComponentRunStates {
    // These two are required basically as a standard way to enable the impl of IoView and
    // WasiView.
    // impl of WasiView is required by [`wasmtime_wasi::p2::add_to_linker_sync`]
    pub wasi_ctx: WasiCtx,
    pub resource_table: ResourceTable,
    // You can add other custom host states if needed
}

impl WasiView for ComponentRunStates {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi_ctx,
            table: &mut self.resource_table,
        }
    }
}

#[cfg(test)]
pub mod test {

    use wasmtime::component::{Component, Linker, ResourceTable};
    use wasmtime::*;
    use wasmtime_wasi::p2::bindings::sync::Command;
    use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};

    pub struct ComponentRunStates {
        // These two are required basically as a standard way to enable the impl of IoView and
        // WasiView.
        // impl of WasiView is required by [`wasmtime_wasi::p2::add_to_linker_sync`]
        pub wasi_ctx: WasiCtx,
        pub resource_table: ResourceTable,
        // You can add other custom host states if needed
    }

    impl WasiView for ComponentRunStates {
        fn ctx(&mut self) -> WasiCtxView<'_> {
            WasiCtxView {
                ctx: &mut self.wasi_ctx,
                table: &mut self.resource_table,
            }
        }
    }

    #[test]
    fn wasi_p2() -> Result<()> {
        // Define the WASI functions globally on the `Config`.
        let engine = Engine::default();
        let mut linker = Linker::new(&engine);
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;

        // Create a WASI context and put it in a Store; all instances in the store
        // share this context. `WasiCtx` provides a number of ways to
        // configure what the target program will have access to.
        let wasi = WasiCtx::builder().inherit_stdio().inherit_args().build();
        let state = ComponentRunStates {
            wasi_ctx: wasi,
            resource_table: ResourceTable::new(),
        };
        let mut store = Store::new(&engine, state);

        // Instantiate our component with the imports we've created, and run it.
        let component = Component::from_file(&engine, "../target/wasm/hello_wasip2.wasm").unwrap();
        let command = Command::instantiate(&mut store, &component, &linker)?;
        let program_result = command.wasi_cli_run().call_run(&mut store)?;
        if program_result.is_err() {
            std::process::exit(1)
        }

        Ok(())
    }
}
