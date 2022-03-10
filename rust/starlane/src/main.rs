#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate tablestream;

#[macro_use]
extern crate tracing;

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
use mesh_portal::version::latest::entity::request::create::Require;
use mesh_portal::version::latest::id::Address;
use tokio::io::AsyncReadExt;
use tracing::error;
use starlane_core::command::cli::{CliClient, outlet};
use starlane_core::command::cli::outlet::Frame;
use starlane_core::command::compose::CommandOp;
use starlane_core::command::parse::{command_line, rec_script_line};
use starlane_core::mechtron::portal_client::launch_mechtron_client;
use starlane_core::mechtron::process::launch_mechtron_process;
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
        .version("0.2.0")
        .author("Scott Williams <scott@mightydevco.com>")
        .about("A Resource Mesh").subcommands(vec![SubCommand::with_name("serve").usage("serve a starlane machine instance").arg(Arg::with_name("with-external").long("with-external").takes_value(false).required(false).default_value("false")).display_order(0),
                                                            SubCommand::with_name("config").subcommands(vec![SubCommand::with_name("set-shell").usage("set the shell that the starlane CLI connects to").arg(Arg::with_name("hostname").required(true).help("the hostname of the starlane instance you wish to connect to")).display_order(0),
                                                                                                                            SubCommand::with_name("get-shell").usage("get the shell that the starlane CLI connects to")]).usage("read or manipulate the cli config").display_order(1).display_order(1),
                                                            SubCommand::with_name("exec").usage("execute a command").args(vec![Arg::with_name("command_line").required(true).help("command line to execute")].as_slice()),
                                                            SubCommand::with_name("script").usage("execute commands in a script").args(vec![Arg::with_name("script_file").required(true).help("the script file to execute")].as_slice()),
                                                            SubCommand::with_name("login").usage("login to server").args(vec![Arg::with_name("hostname").required(true).help("the host to connect to"),Arg::with_name("user").required(true).help("the user address"),Arg::with_name("password").required(true).help("password")].as_slice()),
                                                            SubCommand::with_name("mechtron").usage("launch a mechtron portal client").args(vec![Arg::with_name("server").required(true).help("the portal server to connect to"),
                                                                                                                                                       Arg::with_name("wasm_src").required(true).help("the address of the wasm source"),
                                                                                                                                                      ].as_slice()),

    ]);

    let matches = clap_app.clone().get_matches();

    if let Option::Some(serve) = matches.subcommand_matches("login") {
        println!("LOGIN!!!");
    } else if let Option::Some(serve) = matches.subcommand_matches("serve") {
            let starlane = StarlaneMachine::new("server".to_string()).expect("StarlaneMachine server");

            let layout = match serve.value_of("with-external") {
                Some(value) => {
                    match bool::from_str(value ) {
                        Ok(true) => {
                                ConstellationLayout::standalone_with_external().expect("standalone_with_external")
                        }
                        _ =>{
                            ConstellationLayout::standalone().expect("standalone")
                        }
                    }
                },
                None=> ConstellationLayout::standalone().expect("standalone")
            };

            starlane
                .create_constellation("standalone", layout)
                .await
                .expect("constellation");
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
    } else if let Option::Some(args) = matches.subcommand_matches("mechtron") {
        mechtron(args.clone()).await?;
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
                println!("transferring: '{}'",name.as_str());
                match File::open(name.clone() ) {
                    Ok(mut file) => {
                        let mut buf = vec![];
                        file.read_to_end(&mut buf)?;
                        let bin = Arc::new(buf);
                        exchange.file( name, bin).await?;
                    }
                    Err(err) => {
                        error!("{}",err.to_string())
                    }
                }
            }
            Require::Auth(auth) => {
                println!("RECEIVED AUTH: {}", auth);
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


async fn mechtron(args: ArgMatches<'_>) -> Result<(), Error> {
println!("Staring starlane mechtron process");
   async fn launch(args: ArgMatches<'_>) -> Result<(), Error> {
       let server = args.value_of("server").ok_or("expected server hostname")?.to_string();
       let wasm_src = args.value_of("wasm_src").ok_or("expected Wasm source")?.to_string();
       let wasm_src = Address::from_str(wasm_src.as_str())?;
       launch_mechtron_client( server, wasm_src ).await?;
       Ok(())
   }

    match launch(args).await {
        Ok(_) => {
            std::process::exit(0);
        }
        Err(err) => {
            eprintln!("client launch error: {}",err.to_string());
            std::process::exit(1);
        }
    }


}


pub async fn client() -> Result<CliClient, Error> {
    let host = {
        let config = crate::cli::CLI_CONFIG.lock()?;
        config.hostname.clone()
    };
    CliClient::new(host).await
}
