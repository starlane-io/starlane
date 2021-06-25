use clap::{App, SubCommand, Arg, ArgMatches};
use tokio::runtime::Runtime;

use starlane_core::error::Error;
use starlane_core::starlane::{ConstellationCreate, Starlane, StarlaneCommand};
use starlane_core::template::{ConstellationData, ConstellationTemplate};

mod cli;

#[macro_use]
extern crate lazy_static;

fn main() -> Result<(), Error> {
    let mut clap_app = App::new("Starlane")
        .version("0.1.0")
        .author("Scott Williams <scott@mightydevco.com>")
        .about("A Resource Mesh").subcommands(vec![SubCommand::with_name("run").usage("run an instance of starlane").display_order(0),
                                                            SubCommand::with_name("config").subcommands(vec![SubCommand::with_name("set-host").usage("set the host that the starlane CLI connects to").arg(Arg::with_name("hostname").required(true).help("the hostname of the starlane instance you wish to connect to")).display_order(0),
                                                                                                                            SubCommand::with_name("get-host").usage("get the host that the starlane CLI connects to")]).usage("read or manipulate the cli config").display_order(1).display_order(1),
                                                            SubCommand::with_name("push").usage("push an artifact bundle").args(vec![Arg::with_name("dir").required(true).help("the source directory for this bundle"),
                                                                                                                                                                                       Arg::with_name("address").required(true).help("the publish address of this bundle i.e. 'space:sub_space:bundle:1.0.0'")].as_slice())
    ]);

    let matches = clap_app.clone().get_matches();

    if let Option::Some(_) = matches.subcommand_matches("run") {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let mut starlane = Starlane::new().unwrap();
            let tx = starlane.tx.clone();

            let (command, _) = ConstellationCreate::new(
                ConstellationTemplate::new_standalone_with_mysql(),
                ConstellationData::new(),
                Option::Some("standalone-with-mysql".to_owned()));
            tx.send(StarlaneCommand::ConstellationCreate(command)).await;
            starlane.run().await;
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
println!("push {} to {}", args.value_of("dir").ok_or("expected dir")?, args.value_of("address").ok_or("expected address")? )
    } else {
        clap_app.print_long_help().unwrap_or_default();
    }

    Ok(())
}
