#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate tablestream;


use std::fs::File;
use std::io::{Read, Write};
use std::io;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use clap::{App, Arg, ArgMatches, SubCommand};
use starlane_core::error::Error;
use tracing_subscriber::FmtSubscriber;
use tracing::dispatcher::set_global_default;
use tokio::runtime::Runtime;
use starlane_core::starlane::StarlaneMachine;
use starlane_core::template::ConstellationLayout;
use starlane_core::util::shutdown;
use starlane_core::util;
use starlane_core::starlane::api::StarlaneApi;
use std::convert::TryInto;
use mesh_portal_serde::version::latest::entity::request::create::Require;
use tokio::io::AsyncReadExt;
use starlane_core::command::cli::{CliClient, outlet};
use starlane_core::command::cli::outlet::Frame;
use starlane_core::command::compose::CommandOp;
use starlane_core::command::parse::{command_line, rec_script_line};
use starlane_core::star::shell::sys::SysCall::Create;


pub mod cli;
pub mod resource;


fn main() -> Result<(), Error> {
    let rt = Runtime::new().unwrap();
    rt.block_on( async move { go().await });
    Ok(())
}

async fn go() -> Result<(),Error> {
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
                                                            SubCommand::with_name("config").subcommands(vec![SubCommand::with_name("set-shell").usage("set the shell that the starlane CLI connects to").arg(Arg::with_name("hostname").required(true).help("the hostname of the starlane instance you wish to connect to")).display_order(0),
                                                                                                                            SubCommand::with_name("get-shell").usage("get the shell that the starlane CLI connects to")]).usage("read or manipulate the cli config").display_order(1).display_order(1),
                                                            SubCommand::with_name("exec").usage("execute a command").args(vec![Arg::with_name("command_line").required(true).help("command line to execute")].as_slice()),
                                                            SubCommand::with_name("script").usage("execute commands in a script").args(vec![Arg::with_name("script_file").required(true).help("the script file to execute")].as_slice()),

    ]);

    let matches = clap_app.clone().get_matches();

    if let Option::Some(serve) = matches.subcommand_matches("serve") {
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
    } else if let Option::Some(matches) = matches.subcommand_matches("config") {
        if let Option::Some(_) = matches.subcommand_matches("get-shell") {
            let config = crate::cli::CLI_CONFIG.lock()?;
            println!("{}", config.hostname);
        } else if let Option::Some(args) = matches.subcommand_matches("set-shell") {
            let mut config = crate::cli::CLI_CONFIG.lock()?;
            config.hostname = args
                .value_of("hostname")
                .ok_or("expected hostname")?
                .to_string();
            config.save()?;
        } else {
            clap_app.print_long_help().unwrap_or_default();
        }
    } else if let Option::Some(args) = matches.subcommand_matches("exec") {
        exec(args.clone()).await.unwrap();
    } else if let Option::Some(args) = matches.subcommand_matches("script") {
        match script(args.clone()).await {
            Ok(_) => {
                println!("Script OK");
            }
            Err(err) => {
                eprintln!("Script Error {}", err.to_string() );
            }
        }
    } else {
        clap_app.print_long_help().unwrap_or_default();
    }

    Ok(())
}

async fn exec_command_line(client: CliClient, line: String) -> Result<(CliClient,i32), Error> {
    let op = CommandOp::from_str(line.as_str() )?;
    let requires = op.requires();

    let mut exchange = client.send(line).await?;

    for require in requires {
        match require {
            Require::File(name) => {
                println!("transfering: '{}'",name.as_str());
                let mut file = File::open(name.clone()).unwrap();
                let mut buf = vec![];
                file.read_to_end(&mut buf)?;
                let bin = Arc::new(buf);
                exchange.file( name, bin).await?;
            }
        }
    }

    exchange.end_requires().await?;

    while let Option::Some(Ok(frame)) = exchange.read().await {
        match frame {
            outlet::Frame::StdOut(line) => {
                println!("{}", line);
            }
            outlet::Frame::StdErr(line) => {
                eprintln!("{}", line);
            }
            outlet::Frame::EndOfCommand(code) => {
                return Ok((exchange.into(), code) );
            }
        }
    }
    Err("client disconnected unexpect".into())
}


async fn exec(args: ArgMatches<'_>) -> Result<(), Error> {
    let mut client = client().await?;
    let line = args.value_of("command_line").ok_or("expected command line")?.to_string();

    let (_,code) = exec_command_line(client,line).await?;

    std::process::exit(code);

    Ok(())
}

async fn script(args: ArgMatches<'_>) -> Result<(), Error> {
    let mut client = client().await?;
    let script_file = args.value_of("script_file").ok_or("expected script filename")?.to_string();

    let mut file = File::open(script_file ).unwrap();
    let mut buf = vec![];
    file.read_to_end(&mut buf)?;
    let mut script = String::from_utf8(buf)?;
    loop {
        let (next,line)  = rec_script_line(script.as_str() )?;
        println!("{}",line);
        let (c,code) = exec_command_line(client, line.to_string() ).await?;
        client = c;
        if code != 0 {
            std::process::exit(code);
        }
        script = next.to_string();

        if script.is_empty() {
            break;
        }
    }

    std::process::exit(0);
}


pub async fn client() -> Result<CliClient, Error> {
    let host = {
        let config = crate::cli::CLI_CONFIG.lock()?;
        config.hostname.clone()
    };
    CliClient::new(host).await
}
