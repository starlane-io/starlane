#[macro_use]
extern crate clap;

use clap::arg;
use clap::command;
use clap::{App, Arg, Args, Command, Parser, Subcommand};
use cosmic_hyperlane::test_util::SingleInterchangePlatform;
use cosmic_hyperlane::HyperwayEndpointFactory;
use cosmic_hyperlane_tcp::HyperlaneTcpClient;
use cosmic_hyperverse::driver::control::ControlClient;
use cosmic_universe::err::UniErr;
use cosmic_universe::loc::{Point, ToSurface};
use cosmic_universe::log::RootLogger;
use cosmic_universe::substance::Substance;
use cosmic_universe::wave::core::ReflectedCore;
use std::str::FromStr;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), UniErr> {
    let matches = Command::new("comsic-cli")
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
                .default_value("."),
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
    let platform = SingleInterchangePlatform::new().await;
    let logger = RootLogger::default();
    let logger = logger.point(Point::from_str("client")?);
    let tcp_client: Box<dyn HyperwayEndpointFactory> = Box::new(HyperlaneTcpClient::new(
        format!("{}:{}", host, 4343),
        certs,
        platform.knock(Point::from_str("client")?.to_surface()),
        false,
        logger,
    ));

    let client = ControlClient::new(tcp_client)?;
    client.wait_for_ready(Duration::from_secs(5)).await?;

    tokio::time::sleep(Duration::from_secs(1)).await;
    let cli = client.new_cli_session().await?;

    match cli.exec(command).await {
        Ok(ok) => match ok.body {
            Substance::Text(text) => println!("{}", text),
            Substance::Errors(errors) => {
                for (_, error) in errors.iter() {
                    println!("{}", error)
                }
            }
            s => {
                println!("ResponseKind: {}", s.kind().to_string());
            }
        },
        Err(err) => {
            println!("{}", err.to_string())
        }
    }

    Ok(())
}
