use clap::{App, SubCommand, Arg, ArgMatches, Values};

use tokio::runtime::Runtime;

use starlane_core::error::Error;
use starlane_core::starlane::{ConstellationCreate, StarlaneMachine, StarlaneCommand, StarlaneMachineRunner};
use starlane_core::template::{ConstellationData, ConstellationTemplate, ConstellationLayout};
use tokio::sync::oneshot;
use starlane_core::resource::{ResourceAddress, Version, ResourceSelector, FieldSelection, ResourceCreate, KeyCreationSrc, AddressCreationSrc, ResourceArchetype, ResourceCreateStrategy, AssignResourceStateSrc};
use std::str::FromStr;
use std::{fs, thread};
use std::path::Path;
use std::fs::File;
use std::sync::Arc;
use starlane_core::artifact::ArtifactBundleAddress;
use std::io::Read;
use tracing_subscriber::FmtSubscriber;
use tracing::dispatcher::set_global_default;
use tokio::time::Duration;
use starlane_core::starlane::api::StarlaneApi;
use starlane_core::util::shutdown;
use starlane_core::resource::selector::MultiResourceSelector;
use std::ffi::OsString;
use starlane_core::util;

mod cli;

#[macro_use]
extern crate lazy_static;

fn main() -> Result<(), Error> {
    let subscriber = FmtSubscriber::default();
    set_global_default(subscriber.into()).expect("setting global default tracer failed");

    ctrlc::set_handler( move || {
        std::process::exit(1);
    }).expect("expected to be able to set ctrl-c handler");

    let mut clap_app = App::new("Starlane")
        .version("0.1.0")
        .author("Scott Williams <scott@mightydevco.com>")
        .about("A Resource Mesh").subcommands(vec![SubCommand::with_name("serve").usage("serve a starlane machine instance").display_order(0),
                                                            SubCommand::with_name("config").subcommands(vec![SubCommand::with_name("set-host").usage("set the host that the starlane CLI connects to").arg(Arg::with_name("hostname").required(true).help("the hostname of the starlane instance you wish to connect to")).display_order(0),
                                                                                                                            SubCommand::with_name("get-host").usage("get the host that the starlane CLI connects to")]).usage("read or manipulate the cli config").display_order(1).display_order(1),
                                                            SubCommand::with_name("publish").usage("publish an artifact bundle").args(vec![Arg::with_name("dir").required(true).help("the source directory for this bundle"),Arg::with_name("address").required(true).help("the publish address of this bundle i.e. 'space:sub_space:bundle:1.0.0'")].as_slice()),
                                                            SubCommand::with_name("create").usage("create a resource").setting(clap::AppSettings::TrailingVarArg).args(vec![Arg::with_name("address").required(true).help("address of your new resource"),Arg::with_name("init-args").multiple(true).required(false)].as_slice()),

                                                            SubCommand::with_name("ls").usage("list resources").args(vec![Arg::with_name("address").required(true).help("the resource address to list"),Arg::with_name("child-pattern").required(false).help("a pattern describing the children to be listed .i.e '<File>' for returning resource type File")].as_slice())
    ]);


    let matches = clap_app.clone().get_matches();

    if let Option::Some(_) = matches.subcommand_matches("serve") {
        let rt = Runtime::new().unwrap();
        rt.block_on(async move {
            let mut starlane = StarlaneMachine::new("server".to_string() ).unwrap();
            starlane.create_constellation("standalone", ConstellationLayout::standalone().unwrap()).await.unwrap();
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
    } else if let Option::Some(args) = matches.subcommand_matches("publish") {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            publish(args.clone()).await.unwrap();
        });
        shutdown();
    } else if let Option::Some(args) = matches.subcommand_matches("create") {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            create(args.clone()).await.unwrap();
        });
        shutdown();
    } else if let Option::Some(args) = matches.subcommand_matches("ls") {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            list(args.clone()).await.unwrap();
        });
        shutdown();

    } else {
        clap_app.print_long_help().unwrap_or_default();
    }

    Ok(())
}

async fn publish(args: ArgMatches<'_> ) -> Result<(),Error> {

    let bundle = ArtifactBundleAddress::from_str( args.value_of("address").ok_or("expected address")? )?;

    let input = Path::new(args.value_of("dir").ok_or("expected directory")?);

    let mut zipfile = if input.is_dir() {
        let mut zipfile = tempfile::NamedTempFile::new()?;
        util::zip( args.value_of("dir").expect("expected directory").to_string().as_str(),
                           &zipfile.reopen()?,
                    zip::CompressionMethod::Deflated )?;
        zipfile.reopen()?
    } else {
        File::open(input)?
    };

    let mut data = Vec::with_capacity(zipfile.metadata()?.len() as _ );
    zipfile.read_to_end(&mut data).unwrap();
    let data = Arc::new(data);

    let starlane_api = starlane_api().await?;
    let create_artifact_bundle = starlane_api.create_artifact_bundle(&bundle,data).await?;
    create_artifact_bundle.submit().await?;

    Ok(())
}

async fn list(args: ArgMatches<'_> ) -> Result<(),Error> {
    let address = ResourceAddress::from_str( args.value_of("address").ok_or("expected resource address")? )?;
    let starlane_api = starlane_api().await?;

    let mut selector = if args.value_of("child-pattern" ).is_some(){
        let selector = MultiResourceSelector::from_str( args.value_of("child-pattern").unwrap() )?;
        selector.into()
    } else {
        ResourceSelector::new()
    };

    let resources = starlane_api.select(&address.into(), selector).await?;

    println!();
    for resource in resources {
        println!("{}", resource.stub.address.to_string() );
    }
    println!();

    starlane_api.shutdown();

    Ok(())
}

async fn create(args: ArgMatches<'_> ) -> Result<(),Error> {

    let address = ResourceAddress::from_str( args.value_of("address").ok_or("expected resource address")? )?;
    let resource_type = address.resource_type();
    let kind = resource_type.default_kind()?;

    let init_args = match args.values_of("init-args") {
        None => {"".to_string()}
        Some(args) => {
            let init_args:Vec<&str>  = args.collect();
            let init_args:Vec<String> = init_args.iter().map(|s| (*s).to_string() ).collect();
            init_args.join(" ")
        }
    };

    let starlane_api = starlane_api().await?;

    let create = ResourceCreate {
            parent: address.parent().expect("must have an address with a parent" ).into(),
            key: KeyCreationSrc::None,
            address: AddressCreationSrc::Exact(address.clone()),
            archetype: ResourceArchetype{
                kind: address.resource_type().default_kind()?,
                specific: None,
                config: None
            },
            src: AssignResourceStateSrc::InitArgs(init_args),
            registry_info: Option::None,
            owner: Option::None,
            strategy: ResourceCreateStrategy::Create
    };
    starlane_api.create_resource(create).await?;

    starlane_api.shutdown();

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
