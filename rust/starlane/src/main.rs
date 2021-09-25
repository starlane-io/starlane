#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate tablestream;


use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use clap::{App, Arg, ArgMatches, SubCommand};
use tokio::runtime::Runtime;
use tracing::dispatcher::set_global_default;
use tracing_subscriber::FmtSubscriber;

use starlane_core::error::Error;
use starlane_core::resource::{ResourceAddress, ResourceRecord};
use starlane_core::resource::selector::MultiResourceSelector;
use starlane_core::starlane::{
    ConstellationCreate, StarlaneCommand, StarlaneMachine, StarlaneMachineRunner,
};
use starlane_core::starlane::api::StarlaneApi;
use starlane_core::template::{ConstellationData, ConstellationLayout, ConstellationTemplate};
use starlane_core::util;
use starlane_core::util::shutdown;

use starlane_resources::{ResourceCreate, KeyCreationSrc, AddressCreationSrc, ResourceArchetype, AssignResourceStateSrc, ResourceCreateStrategy, ResourceSelector, ResourcePath, ResourcePathAndKind, ResourceKind, FileKind, ConfigSrc};
use starlane_resources::data::{DataSet, BinSrc, Meta};
use starlane_resources::property::{ResourcePropertyValueSelector, ResourceValueSelector, ResourcePropertyAssignment};
use starlane_core::watch::{WatchResourceSelector, Property};
use starlane_resources::message::MessageFrom;
use starlane_core::frame::StarPattern;
use std::io;
use tablestream::{Stream, Column};
use starlane_core::star::StarKey;
use starlane_core::parse::parse_star_pattern;
use starlane_resources::parse::parse_resource_properties_kind;

mod cli;
mod resource;

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
                                                            SubCommand::with_name("cp").usage("copy a file").args(vec![Arg::with_name("src").takes_value(true).index(1).required(true).help("the source file [local file or starlane resource address]"),Arg::with_name("dst").takes_value(true).index(2).required(true).help("the  destination [local file or starlane resource address]")].as_slice()),
                                                            SubCommand::with_name("create").usage("create a resource").args(vec![Arg::with_name("address").takes_value(true).index(1).required(true).help("resource address"),Arg::with_name("config").takes_value(true).index(2).required(false).help("the  config")].as_slice()),
//                                                            SubCommand::with_name("create").usage("create a resource").args(vec![Arg::with_name("address").index(1).required(true).help("config of your new resource"),Arg::with_name("config").index(2).required(false).takes_value(true).help("configurations")].as_slice()),
                                                            SubCommand::with_name("ls").usage("list resources").args(vec![Arg::with_name("address").required(true).help("the resource address to list"),Arg::with_name("child-pattern").required(false).help("a pattern describing the children to be listed .i.e '<File>' for returning resource type File")].as_slice()),
                                                            SubCommand::with_name("get").usage("get a resource property value").args(vec![Arg::with_name("address").required(true).help("the resource property value")].as_slice()),
                                                            SubCommand::with_name("set").usage("set a resource property value").args(vec![Arg::with_name("address").required(true).help("the resource property value")].as_slice()),
                                                            SubCommand::with_name("stars").usage("stars subcommand").args(vec![Arg::with_name("star-pattern").index(1).required(false).help("the star pattern to list"),Arg::with_name("resource-properties").index(2).required(false)].as_slice()),
                                                            SubCommand::with_name("watch").usage("watch resources property value for changes").args(vec![Arg::with_name("address").required(true).help("the resource property value to watch")].as_slice())
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
    } else if let Option::Some(args) = matches.subcommand_matches("cp") {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            cp(args.clone()).await.unwrap();
        });
        shutdown();
    }
    else if let Option::Some(args) = matches.subcommand_matches("create") {
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
    } else if let Option::Some(args) = matches.subcommand_matches("set") {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            set(args.clone()).await.unwrap();
        });
        shutdown();
    }
 else if let Option::Some(args) = matches.subcommand_matches("get") {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            get(args.clone()).await.unwrap();
        });
        shutdown();
    } else if let Option::Some(args) = matches.subcommand_matches("watch") {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            watch(args.clone()).await.unwrap();
        });
        shutdown();
    }
     else if let Option::Some(args) = matches.subcommand_matches("stars") {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
        stars(args.clone()).await.unwrap();
        });
        shutdown();
    } else {
        clap_app.print_long_help().unwrap_or_default();
    }

    Ok(())
}

async fn publish(args: ArgMatches<'_>) -> Result<(), Error> {
    let bundle = ResourcePath::from_str(args.value_of("address").ok_or("expected address")?)?;

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
    let create = starlane_api.create_artifact_bundle(bundle, data).await?;
    create.submit().await?;

    Ok(())
}

async fn cp(args: ArgMatches<'_>) -> Result<(), Error> {

    let starlane_api = starlane_api().await?;

    let src = args.value_of("src").ok_or("expected src")?;
    let dst = args.value_of("dst").ok_or( "expected dst")?;

    if dst.contains(":") {
        let dst = ResourcePath::from_str(dst)?;
        let src = Path::new(src );
        // copying from src to dst
        let mut src = File::open(src )?;
        let mut content= Vec::with_capacity(src.metadata()?.len() as _);
        src.read_to_end(&mut content ).unwrap();
        let content = Arc::new(content);
        let content = BinSrc::Memory(content);
        let mut state = DataSet::new();
        state.insert("content".to_string(), content );

        let meta = Meta::new();
        let meta = BinSrc::Memory(Arc::new(meta.bin()?));
        state.insert("meta".to_string(), meta );

        let create = ResourceCreate {
            parent: dst
                .parent()
                .ok_or("must have an address with a parent")?
                .into(),
            key: KeyCreationSrc::None,
            address: AddressCreationSrc::Exact(dst),
            archetype: ResourceArchetype {
                kind: ResourceKind::File(FileKind::File),
                specific: None,
                config: ConfigSrc::None,
            },
            state_src: AssignResourceStateSrc::Direct(state),
            registry_info: Option::None,
            owner: Option::None,
            strategy: ResourceCreateStrategy::CreateOrUpdate,
            from: MessageFrom::Inject
        };

        starlane_api.create_resource(create).await?;

        starlane_api.shutdown();

    } else  if src.contains(":") {
      let src = ResourcePath::from_str(src)?;
      let content = starlane_api.get_resource_state(src.into()).await?.remove("content").expect("expected 'content' state aspect");
      let filename = dst.clone();
      let dst = Path::new(dst );
      let mut dst = File::create(dst).expect(format!("could not open file for writing: {}", filename ).as_str() );
      match content {
          BinSrc::Memory(bin) => {
              dst.write_all(bin.as_slice() ).expect(format!("could not write to file: {}", filename ).as_str() )
          }
      }
    } else {
        unimplemented!("copy from starlane to local not yet supported")
    }

    Ok(())
}

async fn list(args: ArgMatches<'_>) -> Result<(), Error> {
    let address = ResourcePath::from_str(
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

    for resource in resources {
        println!("{}", resource.stub.address.to_string());
    }

    starlane_api.shutdown();

    Ok(())
}

async fn create(args: ArgMatches<'_>) -> Result<(), Error> {



    let address = ResourcePathAndKind::from_str(
        args.value_of("address")
            .ok_or("expected resource address")?,
    )?;

    let config = match args.value_of("config") {
        None => {
println!("NO CONFIG {} ", address.to_string()  );
            ConfigSrc::None
        }
        Some(artifact) => {
println!("CONFIG! {}", address.to_string());
            ConfigSrc::Artifact(ResourcePath::from_str(artifact)?)
        }
    };


    let kind = address.kind.clone();
    let address: ResourcePath = address.into();

    /*
    let create_args = match args.values_of("create-args") {
        None => "".to_string(),
        Some(args) => {
            let create_args: Vec<&str> = args.collect();
            let create_args: Vec<String> = create_args.iter().map(|s| (*s).to_string()).collect();
            create_args.join(" ")
        }
    };

     */

    let create_args= "".to_string();

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
            config: config
        },
        state_src: AssignResourceStateSrc::CreateArgs(create_args),
        registry_info: Option::None,
        owner: Option::None,
        strategy: ResourceCreateStrategy::Create,
        from: MessageFrom::Inject
    };

    starlane_api.create_resource(create).await?;

    starlane_api.shutdown();

    Ok(())
}

async fn get(args: ArgMatches<'_>) -> Result<(), Error> {
    let address = ResourceValueSelector::from_str(
        args.value_of("address")
            .ok_or("expected resource property value address")?,
    )?;
    let starlane_api = starlane_api().await?;

    let values = starlane_api.select_values(address.resource, address.property).await?;

    for (k,v) in values.values {
        println!("{}",v.to_string());
    }

    starlane_api.shutdown();

    Ok(())
}

async fn set(args: ArgMatches<'_>) -> Result<(), Error> {

    let assignment = ResourcePropertyAssignment::from_str(
        args.value_of("address").ok_or("expected resource property assignment")?,
    )?;

    let starlane_api = starlane_api().await?;

    starlane_api.set_property(assignment).await?;

    starlane_api.shutdown();

    Ok(())
}

async fn watch(args: ArgMatches<'_>) -> Result<(), Error> {
    let address = ResourcePath::from_str(
        args.value_of("address")
            .ok_or("expected resource property value address")?,
    )?;
    let starlane_api = starlane_api().await?;

    let selector = WatchResourceSelector::new( address.into(), Property::State );

    let mut listener = starlane_api.watch(selector).await?;

    while let Option::Some(notification)  = listener.rx.recv().await {
        for change in notification.changes {
            println!("received notification: {}", change.to_string() );
        }
    }

    starlane_api.shutdown();

    Ok(())
}

async fn stars(args: ArgMatches<'_>) -> Result<(), Error> {

    let star_pattern = if args.is_present("star-pattern") {
        let star_pattern = args.value_of("star-pattern")
            .ok_or("expected star_pattern")?;
        parse_star_pattern(star_pattern.trim())?.1?
    } else {
        StarPattern::Any
    };


    let starlane_api = starlane_api().await?;



    let hits = starlane_api.star_search(star_pattern).await?;
    match args.value_of("resource-properties") {
        None => {
            let mut out = io::stdout();

            let mut stream = Stream::new( &mut out, vec![
                Column::new(  |f, k:&(StarKey,usize)| write!(f, "{}", k.0.to_string() )).header("star"),
                Column::new(  |f, k:&(StarKey,usize)| write!(f, "{}", k.1 )).header("hops"),
            ] );

            for (star,hops) in hits.hits {
                stream.row((star,hops));
            }
            stream.finish();
        }
        Some(kind) => {
            let (_,kind) = parse_resource_properties_kind(kind.trim())?;


            let mut out = io::stdout();


            let mut stream = Stream::new( &mut out, vec![
                Column::new(  |f, r:&ResourceRecord| write!(f, "{}", r.stub.key.to_string() )).header("key"),
                Column::new(  |f, r:&ResourceRecord| write!(f, "{}", r.stub.address.to_string() )).header("address"),
                Column::new(  |f, r:&ResourceRecord| write!(f, "{}", r.stub.archetype.kind.to_string() )).header("kind"),
                Column::new(  |f, r:&ResourceRecord| write!(f, "{}", r.stub.archetype.config.to_string() )).header("config"),
            ] );

            for (star,hops) in hits.hits {
                let selector = ResourceSelector::new();
                let records = starlane_api.select_from_star(star, selector).await?;
                for record in records {
                    stream.row(record);
                }
            }
            stream.finish();
        }
    }


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
    starlane.create_constellation("client", layout).await?;
    Ok(starlane.get_starlane_api().await?)
}
