use clap::{App, SubCommand, Arg, ArgMatches};
use tokio::runtime::Runtime;

use starlane_core::error::Error;
use starlane_core::starlane::{ConstellationCreate, StarlaneMachine, StarlaneCommand, StarlaneMachineRunner};
use starlane_core::template::{ConstellationData, ConstellationTemplate, ConstellationLayout};
use tokio::sync::oneshot;
use starlane_core::resource::{ResourceAddress, Version};
use std::str::FromStr;
use std::fs;
use std::path::Path;
use std::fs::File;
use std::sync::Arc;
use starlane_core::artifact::ArtifactBundleAddress;
use std::io::Read;
use tracing_subscriber::FmtSubscriber;
use tracing::dispatcher::set_global_default;
use tokio::time::Duration;
use starlane_core::starlane::api::StarlaneApi;

mod cli;

#[macro_use]
extern crate lazy_static;

fn main() -> Result<(), Error> {
    let subscriber = FmtSubscriber::default();
    set_global_default(subscriber.into()).expect("setting global default tracer failed");

    let mut clap_app = App::new("Starlane")
        .version("0.1.0")
        .author("Scott Williams <scott@mightydevco.com>")
        .about("A Resource Mesh").subcommands(vec![SubCommand::with_name("server").usage("run an instance of starlane server").display_order(0),
                                                            SubCommand::with_name("config").subcommands(vec![SubCommand::with_name("set-host").usage("set the host that the starlane CLI connects to").arg(Arg::with_name("hostname").required(true).help("the hostname of the starlane instance you wish to connect to")).display_order(0),
                                                                                                                            SubCommand::with_name("get-host").usage("get the host that the starlane CLI connects to")]).usage("read or manipulate the cli config").display_order(1).display_order(1),
                                                            SubCommand::with_name("push").usage("push an artifact bundle").args(vec![Arg::with_name("dir").required(true).help("the source directory for this bundle"),
                                                                                                                                                                                       Arg::with_name("address").required(true).help("the publish address of this bundle i.e. 'space:sub_space:bundle:1.0.0'")].as_slice())
    ]);

    let matches = clap_app.clone().get_matches();

    if let Option::Some(_) = matches.subcommand_matches("server") {
        let rt = Runtime::new().unwrap();
        rt.block_on(async move {
            let mut starlane = StarlaneMachine::new("server".to_string() ).unwrap();
            starlane.create_constellation("standalone", ConstellationLayout::standalone_with_database().unwrap()).await.unwrap();
            starlane.listen().await.expect("expected listen to work");
            starlane.join().await;
        });
    } else if let Option::Some(matches) = matches.subcommand_matches("config") {
        if let Option::Some(_) = matches.subcommand_matches("get-host") {
            let config = crate::cli::CLI_CONFIG.lock()?;
            println!("{}",config.hostname);
        }
        else if let Option::Some(args) = matches.subcommand_matches("set-host") {
            let mut config = crate::cli::CLI_CONFIG.lock()?;
            config.hostname = args.value_of("hostname").ok_or("expected hostname")?.to_string();
            config.save()?;
        }
        else{
            clap_app.print_long_help().unwrap_or_default();
        }
    } else if let Option::Some(args) = matches.subcommand_matches("push") {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            push(args.clone()).await.unwrap();
        });

    } else {
        clap_app.print_long_help().unwrap_or_default();
    }

    Ok(())
}

async fn push( args: ArgMatches<'_> ) -> Result<(),Error> {
    let bundle = ArtifactBundleAddress::from_str( args.value_of("address").ok_or("expected address")? )?;

    let input = Path::new(args.value_of("dir").ok_or("expected directory")?);

    let mut zipfile = if input.is_dir() {
        let mut zipfile = tempfile::tempfile()?;
        let mut zip = zip::ZipWriter::new(zipfile.try_clone()?);

        let input = input.to_str().ok_or("blah")?.to_string();
        zip.add_directory(input,Default::default() )?;
        zip.finish()?;
        zipfile
    } else {
        File::open(input)?
    };

    let mut data = vec![];
    zipfile.read_to_end(&mut data).unwrap();
    let data = Arc::new(data);

    let starlane_api = starlane_api().await?;
    println!("creating.");
    let create_artifact_bundle = starlane_api.create_artifact_bundle(&bundle,data).await?;
    println!("submitting.");
    create_artifact_bundle.submit().await?;
    println!("done");
    Ok(())
}


pub async fn starlane_api() -> Result<StarlaneApi,Error>{
    let mut starlane = StarlaneMachine::new("client".to_string() ).unwrap();
    let mut  layout = ConstellationLayout::client("host".to_string())?;
    let host = {
        let config = crate::cli::CLI_CONFIG.lock()?;
        config.hostname.clone()
    };
    layout.set_machine_host_address("host".to_string(), host );
    starlane.create_constellation( "client", layout ).await?;
    Ok(starlane.get_starlane_api().await?)
}
