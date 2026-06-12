use anyhow::Result;
use wasmtime::{
    Config,
    Engine,
    Store,
};
use wasmtime::component::{Component, Linker};



mod state {
    use starlane_space::particle::Status;
    struct HostState {
        pub status_tx: tokio::sync::watch::Sender<Status>
    }   
}



    

/*


fn main() -> Result<()> {
    let mut config = Config::new();
    config.wasm_component_model(true);

    let engine = Engine::new(&config)?;

    let component =
        Component::from_file(&engine, "guest_component.wasm")?;

    let mut linker = Linker::<MyState>::new(&engine);

    bindings::example::calculator::calculator::add_to_linker(
        &mut linker,
        |state| state,
    )?;

    let mut store = Store::new(&engine, MyState);

    let (_instance, _) =
        bindings::App::instantiate(
            &mut store,
            &component,
            &linker,
        )?;

    Ok(())
}

 */