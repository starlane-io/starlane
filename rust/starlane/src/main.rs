use clap::App;
use tokio::runtime::Runtime;
use starlane_core::starlane::{Starlane, ConstellationCreate, StarlaneCommand};
use starlane_core::template::{ConstellationTemplate, ConstellationData};
use starlane_core::error::Error;

fn main() -> Result<(), Error> {
    let matches = App::new("Starlane")
        .version("0.1.0")
        .author("Scott Williams <scott@mightydevco.com>")
        .about("A Resource Mesh");

    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        let mut starlane = Starlane::new().unwrap();
        let tx = starlane.tx.clone();

        let (command, mut rx) = ConstellationCreate::new(
            ConstellationTemplate::new_standalone(),
            ConstellationData::new(),
            Option::Some("standalone".to_owned()),
        );
        tx.send(StarlaneCommand::ConstellationCreate(command)).await;
        starlane.run().await;
        rx.await;
    });

    Ok(())
}
