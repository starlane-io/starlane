#[macro_use]
extern crate lazy_static;

use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use clap::{App, Arg, ArgMatches, SubCommand};
use tokio::runtime::Runtime;

use tracing::dispatcher::set_global_default;
use tracing_subscriber::FmtSubscriber;

use starlane_core::error::Error;
use starlane_core::resource::selector::MultiResourceSelector;
use starlane_core::resource::ResourceAddressKind;
use starlane_core::resource::{
    AddressCreationSrc, ArtifactBundlePath, AssignResourceStateSrc, KeyCreationSrc,
    ResourceAddress, ResourceArchetype, ResourceCreate, ResourceCreateStrategy, ResourceSelector,
};

use starlane_core::starlane::api::StarlaneApi;
use starlane_core::starlane::{
    ConstellationCreate, StarlaneCommand, StarlaneMachine, StarlaneMachineRunner,
};
use starlane_core::template::{ConstellationData, ConstellationLayout, ConstellationTemplate};
use starlane_core::util;
use starlane_core::util::shutdown;

mod cli;

fn main() -> Result<(), Error> {
    let subscriber = FmtSubscriber::default();
    set_global_default(subscriber.into()).expect("setting global default tracer failed");

    ctrlc::set_handler(move || {
        std::process::exit(1);
    })
    .expect("expected to be able to set ctrl-c handler");

    let mut clap_app = App::new("Starlane")
        .version("0.1.0")
        .author("Scott Williams <scott@mightydevco.com>")
        .about("A Resource Mesh").subcommands(vec![SubCommand::with_name("serve").usage("serve a starlane machine instance").arg(Arg::with_name("with-external").long("with-external").takes_value(false).required(false)).display_order(0),
                                                            SubCommand::with_name("config").subcommands(vec![SubCommand::with_name("set-host").usage("set the host that the starlane CLI connects to").arg(Arg::with_name("hostname").required(true).help("the hostname of the starlane instance you wish to connect to")).display_order(0),
                                                                                                                            SubCommand::with_name("get-host").usage("get the host that the starlane CLI connects to")]).usage("read or manipulate the cli config").display_order(1).display_order(1),
                                                            SubCommand::with_name("publish").usage("publish an artifact bundle").args(vec![Arg::with_name("dir").required(true).help("the source directory for this bundle"),Arg::with_name("address").required(true).help("the publish address of this bundle i.e. 'space:sub_space:bundle:1.0.0'")].as_slice()),
                                                            SubCommand::with_name("create").usage("create a resource").setting(clap::AppSettings::TrailingVarArg).args(vec![Arg::with_name("address").required(true).help("address of your new resource"),Arg::with_name("create-args").multiple(true).required(false)].as_slice()),

                                                            SubCommand::with_name("ls").usage("list resources").args(vec![Arg::with_name("address").required(true).help("the resource address to list"),Arg::with_name("child-pattern").required(false).help("a pattern describing the children to be listed .i.e '<File>' for returning resource type File")].as_slice())
    ]);

    let matches = clap_app.clone().get_matches();

    if let Option::Some(serve) = matches.subcommand_matches("serve") {
        let rt = Runtime::new().unwrap();
        rt.block_on(async move {
            let starlane = StarlaneMachine::new("server".to_string()).unwrap();
            let layout = match serve.is_present("with-external") {
                false => ConstellationLayout::standalone().unwrap(),
                true => ConstellationLayout::standalone_with_external().unwrap(),
            };

            starlane
                .create_constellation("standalone", layout)
                .await
                .unwrap();
            starlane.listen().await.expect("expected listen to work");
            starlane.join().await;
        });
    } else if let Option::Some(matches) = matches.subcommand_matches("config") {
        if let Option::Some(_) = matches.subcommand_matches("get-host") {
            let config = crate::cli::CLI_CONFIG.lock()?;
            println!("{}", config.hostname);
        } else if let Option::Some(args) = matches.subcommand_matches("set-host") {
            let mut config = crate::cli::CLI_CONFIG.lock()?;
            config.hostname = args
                .value_of("hostname")
                .ok_or("expected hostname")?
                .to_string();
            config.save()?;
        } else {
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

async fn publish(args: ArgMatches<'_>) -> Result<(), Error> {
    let bundle = ArtifactBundlePath::from_str(args.value_of("address").ok_or("expected address")?)?;

    let input = Path::new(args.value_of("dir").ok_or("expected directory")?);

    let mut zipfile = if input.is_dir() {
        let zipfile = tempfile::NamedTempFile::new()?;
        util::zip(
            args.value_of("dir")
                .expect("expected directory")
                .to_string()
                .as_str(),
            &zipfile.reopen()?,
            zip::CompressionMethod::Deflated,
        )?;
        zipfile.reopen()?
    } else {
        File::open(input)?
    };

    let mut data = Vec::with_capacity(zipfile.metadata()?.len() as _);
    zipfile.read_to_end(&mut data).unwrap();
    let data = Arc::new(data);

    let starlane_api = starlane_api().await?;
    starlane_api.create_artifact_bundle(&bundle, data).await?;

    Ok(())
}

async fn list(args: ArgMatches<'_>) -> Result<(), Error> {
    let address = ResourceAddress::from_str(
        args.value_of("address")
            .ok_or("expected resource address")?,
    )?;
    let starlane_api = starlane_api().await?;

    let selector = if args.value_of("child-pattern").is_some() {
        let selector = MultiResourceSelector::from_str(args.value_of("child-pattern").unwrap())?;
        selector.into()
    } else {
        ResourceSelector::new()
    };

    let resources = starlane_api.select(&address.into(), selector).await?;

    println!();
    for resource in resources {
        println!("{}", resource.stub.address.to_string());
    }
    println!();

    starlane_api.shutdown();

    Ok(())
}

async fn create(args: ArgMatches<'_>) -> Result<(), Error> {
    let address = ResourceAddressKind::from_str(
        args.value_of("address")
            .ok_or("expected resource address")?,
    )?;
    let kind = address.kind().clone();
    let address: ResourceAddress = address.into();

    let create_args = match args.values_of("create-args") {
        None => "".to_string(),
        Some(args) => {
            let create_args: Vec<&str> = args.collect();
            let create_args: Vec<String> = create_args.iter().map(|s| (*s).to_string()).collect();
            create_args.join(" ")
        }
    };

    let starlane_api = starlane_api().await?;

    let create = ResourceCreate {
        parent: address
            .parent()
            .ok_or("must have an address with a parent")?
            .into(),
        key: KeyCreationSrc::None,
        address: AddressCreationSrc::Exact(address),
        archetype: ResourceArchetype {
            kind: kind,
            specific: None,
            config: None,
        },
        state_src: AssignResourceStateSrc::CreateArgs(create_args),
        registry_info: Option::None,
        owner: Option::None,
        strategy: ResourceCreateStrategy::Create,
    };
    starlane_api.create_resource(create).await?;

    starlane_api.shutdown();

    Ok(())
}

pub async fn starlane_api() -> Result<StarlaneApi, Error> {
    let starlane = StarlaneMachine::new("client".to_string()).unwrap();
    let mut layout = ConstellationLayout::client("host".to_string())?;
    let host = {
        let config = crate::cli::CLI_CONFIG.lock()?;
        config.hostname.clone()
    };
    layout.set_machine_host_address("host".to_string(), host);
    println!("getting ready to create constellation...");
    starlane.create_constellation("client", layout).await?;
    println!("client constellation created.");
    Ok(starlane.get_starlane_api().await?)
}
