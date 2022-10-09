#![allow(warnings)]

#[macro_use]
extern crate clap;

use clap::arg;
use clap::command;
use clap::{App, Arg, Args, Command as ClapCommand, Parser, Subcommand};
use cosmic_hyperlane::test_util::SingleInterchangePlatform;
use cosmic_hyperlane::HyperwayEndpointFactory;
use cosmic_hyperlane_tcp::HyperlaneTcpClient;
use cosmic_hyperspace::driver::control::ControlClient;
use cosmic_space::err::UniErr;
use cosmic_space::loc::{Point, ToSurface};
use cosmic_space::log::RootLogger;
use cosmic_space::substance::Substance;
use cosmic_space::wave::core::ReflectedCore;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs;
use cosmic_space::command::{CmdTransfer, Command, RawCommand};
use cosmic_space::hyper::{InterchangeKind, Knock};
use cosmic_space::parse::{command_line, upload_blocks};
use cosmic_space::parse::error::result;
use cosmic_space::util::{log, ToResolved};
use cosmic_nom::new_span;

#[tokio::main]
async fn main() -> Result<(), UniErr> {
    let matches = ClapCommand::new("comsic-cli")
        .arg(
            Arg::new("host")
                .short('h')
                .long("host")
                .takes_value(true)
                .value_name("host")
                .required(false)
                .default_value("localhost"),
        )
        .arg(
            Arg::new("certs")
                .short('c')
                .long("certs")
                .takes_value(true)
                .value_name("certs")
                .required(false)
                .default_value("./certs"),
        )
        .allow_external_subcommands(true)
        .get_matches();

    if matches.subcommand_name().is_some() {
        let host = matches.get_one::<String>("host").unwrap().clone();
        let certs = matches.get_one::<String>("certs").unwrap().clone();
        command(host, certs, matches.subcommand_name().unwrap()).await
    } else {
        Ok(())
    }
}

async fn command(host: String, certs: String, command: &str) -> Result<(), UniErr> {
    let logger = RootLogger::default();
    let logger = logger.point(Point::from_str("cosmic-cli")?);
    let tcp_client: Box<dyn HyperwayEndpointFactory> = Box::new(HyperlaneTcpClient::new(
        format!("{}:{}", host, 4343),
        certs,
        Knock::default(),
        false,
        logger,
    ));

    let client = ControlClient::new(tcp_client)?;
    client.wait_for_ready(Duration::from_secs(5)).await?;

    tokio::time::sleep(Duration::from_secs(1)).await;
    let cli = client.new_cli_session().await?;


    let blocks = result(upload_blocks(new_span(command)))?;
    let mut command = RawCommand::new(command.to_string());
    for block in blocks {
        let content = Arc::new(fs::read(block.name.as_str()).await?);
        command.transfers.push( CmdTransfer::new(block.name, content ));
    }

    let core = cli.raw(command).await?;
    core_out(core);

    Ok(())
}

pub fn core_out( core: ReflectedCore ) {
    match core.is_ok() {
        true => out(core.body),
        false => {out_err(core.ok_or().unwrap_err() )}
    }
}

pub fn out(  substance: Substance ) {
    match substance {
        Substance::Empty => {
            println!("Ok");
        }
        Substance::List(list) => {
            for i in list.list {
                out(*i);
            }
        }
        Substance::Point(point) => {
            println!("{}",point.to_string());
        }
        Substance::Surface(surface) => {
            println!("{}",surface.to_string());
        }
        Substance::Text(text) => {
            println!("{}",text);
        }
        Substance::Stub(stub) => {
            println!("{}<{}>", stub.point.to_string(), stub.kind.to_string())
        }
        what => {
            eprintln!("cosmic-cli not sure how to output {}",what.kind().to_string())
        }
    }
}

pub fn out_err( err: UniErr ) {
    eprintln!("{}",err.to_string())
}
