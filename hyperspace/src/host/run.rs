use wasmtime::{Caller, Config, Engine, Linker, Module, Store};
use wasmtime_wasi::WasiCtxBuilder;

pub struct RunHost {

}





#[cfg(test)]
pub mod test {
        /*
        #[test]
        pub fn wasi_p1() -> anyhow::Result<()> {
                // Modules can be compiled through either the text or binary format
                let engine = Engine::default();
                let wat = r#"
        (module
            (import "host" "host_func" (func $host_hello (param i32)))

            (func (export "hello")
                i32.const 3
                call $host_hello)
        )
    "#;
                let module = Module::new(&engine, wat)?;

                // Create a `Linker` which will be later used to instantiate this module.
                // Host functionality is defined by name within the `Linker`.
                let mut linker = Linker::new(&engine);
                linker.func_wrap("host", "host_func", |caller: Caller<'_, u32>, param: i32| {
                        println!("Got {} from WebAssembly", param);
                        println!("my host state is: {}", caller.data());
                })?;

                // All wasm objects operate within the context of a "store". Each
                // `Store` has a type parameter to store host-specific data, which in
                // this case we're using `4` for.
                let mut store = Store::new(&engine, 4);
                let instance = linker.instantiate(&mut store, &module)?;
                let hello = instance.get_typed_func::<(), ()>(&mut store, "hello")?;

                // And finally we can call the wasm!
                hello.call(&mut store, ())?;

                Ok(())
        }

         */



        /*
        You can execute this example with:
            cargo build --target wasm32-wasip2 -p example-wasi-wasm
            cargo run --example wasip2
        */

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