#![allow(warnings)]
#![allow(unused)]

use starlane_package::cache::PackageCache;
use starlane_package::PackFile;
use std::str::FromStr;
use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::{Engine, Result, Store};
use wasmtime_wasi::p2::bindings::Command;
use wasmtime_wasi::{WasiCtx, WasiView};
use starlane_host::exec::{ExecState, Executor, HostService};

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

}

#[cfg(test)]
mod tests {
    use std::any::Any;
    use std::str::FromStr;
    use anyhow::Result;
    use wasmtime::component::{Component, HasData, HasSelf, Linker, ResourceTable};
    use wasmtime::{Engine, Store};
    use wasmtime_wasi::p2::pipe::{MemoryInputPipe, MemoryOutputPipe};
    use wasmtime_wasi::WasiCtx;
    use starlane_host::exec::ExecState;
    use starlane_package::PackFile;
    use crate::tests::bindings::Filter;

    pub mod bindings {
        wasmtime::component::bindgen!({
            path: "../wit",
            world: "filter",
        });
    }

    #[derive(Default)]
    struct MyState {
        ctx: WasiCtx,
        table: ResourceTable
    }

    mod blah {
        use wasmtime::component::HasData;
        use wasmtime_wasi::{WasiCtxView, WasiView};
        use crate::tests::bindings::starlane::hyperspace::space;
        use super::bindings::starlane::hyperspace::status_api::{Host, Status};

        use crate::tests::MyState;

        impl space::Host for MyState {}

        impl Host for MyState {
            fn update(&mut self, status: Status) -> () {
                todo!()
            }
        }

        impl WasiView for MyState {
            fn ctx(&mut self) -> WasiCtxView<'_> {
                WasiCtxView {
                    ctx: &mut self.ctx,
                    table: &mut self.table,
                }
            }
        }

    }




    #[tokio::test]
    async fn test() -> Result<()> {
        let engine = Engine::default();

        let file = PackFile::from_str("starlane.app:examples:0.1.0/email_validator_util.wasm")?;
        let cache = starlane_package::cache::cache_singleton();
        let path = cache.get_path(&file).await.unwrap();
        let component = Component::from_file(&engine, path)?;
        let mut wasi = WasiCtx::builder();
        wasi.inherit_stdio();
        wasi.inherit_stdout();

        let ctx = wasi.build();
        let state = MyState{
            ctx,
            table: ResourceTable::new(),
        };


        let mut linker = Linker::new(&engine);
        Filter::add_to_linker::<_,HasSelf<_>>(&mut linker, |state: &mut MyState| state)?;
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker).unwrap();

        let mut store = Store::new(&engine, state);

        println!("Blah");

        let bindings = Filter::instantiate(&mut store, &component, &linker)?;

        let blah = bindings.starlane_hyperspace_filter_api().call_filter( &mut store, "scottmightydevco.com")?.unwrap();
        println!("answer: '{}'",blah);
        Ok(())
    }
}