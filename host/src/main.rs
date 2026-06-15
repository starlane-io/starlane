#![allow(warnings)]
#![allow(unused)]

use starlane_host::{ExecState, Executor, HostService};
use starlane_package::cache::PackageCache;
use starlane_package::PackFile;
use std::str::FromStr;
use std::thread;
use std::time::Duration;
use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::{Config, Engine, Result, Store};
use wasmtime_wasi::p2::bindings::Command;
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};

// This example is an example shim of executing a component based on the
// command line arguments provided to this program.
#[tokio::main]
async fn _main() -> Result<()> {
    let file = PackFile::from_str("starlane.app:examples:0.1.0/hello_wasip2.wasm")?;
    let cache = starlane_package::cache::cache_singleton();
    let path = cache.get_path(&file).await.unwrap();
    let args = std::env::args().skip(1).collect::<Vec<_>>();

    // Configure and create `Engine`
    let engine = Engine::default();

    // Configure a `Linker` with WASI, compile a component based on
    // command line arguments, and then pre-instantiate it.
    let mut linker = Linker::<ExecState>::new(&engine);
    wasmtime_wasi::p2::add_to_linker_async(&mut linker)?;
    let component = Component::from_file(&engine, &path)?;

    // Configure a `WasiCtx` based on this program's environment. Then
    // build a `Store` to instantiate into.
    let mut builder = WasiCtx::builder();
    builder.inherit_stdio().inherit_env().args(&args);
    let mut store = Store::new(
        &engine,
        ExecState {
            ctx: builder.build(),
            table: ResourceTable::new(),
        },
    );

    // Instantiate the component and we're off to the races.
    let command = Command::instantiate_async(&mut store, &component, &linker).await?;
    let program_result = command.wasi_cli_run().call_run(&mut store).await?;
    match program_result {
        Ok(()) => Ok(()),
        Err(()) => std::process::exit(1),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let service = HostService::new();
    let executor = service
        .executor(&PackFile::from_str("starlane.app:examples:0.1.0/hello_wasip2.wasm").unwrap())
        .await?;

    let blah =  executor.run("Scott").await?;

    println!("BLAH -> {}",blah);

    Ok(())
}

#[cfg(test)]
pub mod test {

    use wasmtime::component::{Component, Linker, ResourceTable};
    use wasmtime_wasi::p2::bindings::Command;
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
        let command = Command::instantiate(&mut store, &component, &linker)?;
        //        command.wasi_cli_run().call_run(&mut store)?;
        //        command.wasi_cli_run().call_run(&mut store)?;

        Ok(())
    }
}
