#![allow(warnings)]

#[macro_use]
extern crate clap;

#[macro_use]
extern crate text_io;

use clap::arg;
use clap::command;
use clap::{App, Arg, Args, Command as ClapCommand, Parser, Subcommand};
use cosmic_hyperlane::test_util::SingleInterchangePlatform;
use cosmic_hyperlane::HyperwayEndpointFactory;
use cosmic_hyperlane_tcp::HyperlaneTcpClient;
use cosmic_hyperspace::driver::control::{ControlCliSession, ControlClient};
use cosmic_nom::new_span;
use cosmic_space::command::{CmdTransfer, Command, RawCommand};
use cosmic_space::err::SpaceErr;
use cosmic_space::hyper::{InterchangeKind, Knock};
use cosmic_space::loc::ToSurface;
use cosmic_space::log::RootLogger;
use cosmic_space::parse::error::result;
use cosmic_space::parse::{command_line, upload_blocks};
use cosmic_space::point::Point;
use cosmic_space::substance::Substance;
use cosmic_space::util::{log, ToResolved};
use cosmic_space::wave::core::ReflectedCore;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use std::io::{Cursor, Read, Write};
use walkdir::{DirEntry, WalkDir};
use zip::{result::ZipError, write::FileOptions};

pub struct Session {
    pub client: ControlClient,
    pub cli: ControlCliSession,
}

impl Session {
    pub async fn new(host: String, certs: String) -> Result<Self, SpaceErr> {
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
        client.wait_for_ready(Duration::from_secs(10)).await?;
        client.wait_for_greet().await?;

        let cli = client.new_cli_session().await?;

        Ok(Self { client, cli })
    }

    async fn command(&self, command: &str) -> Result<(), SpaceErr> {
        let blocks = result(upload_blocks(new_span(command)))?;
        let mut command = RawCommand::new(command.to_string());
        for block in blocks {
            let path = block.name.clone();
            let metadata = std::fs::metadata(&path)?;

            let content = if metadata.is_dir() {
                let file = Cursor::new(Vec::new());

                let walkdir = WalkDir::new(&path);
                let it = walkdir.into_iter();

                let data = match starlane::zip_dir(
                    &mut it.filter_map(|e| e.ok()),
                    &path,
                    file,
                    zip::CompressionMethod::Deflated,
                ) {
                    Ok(data) => data,
                    Err(e) => return Err(SpaceErr::new(500, e.to_string())),
                };

                // return the inner buffer from the cursor
                let data = data.into_inner();
                data
            } else {
                std::fs::read(block.name.as_str())?
            };

            command
                .transfers
                .push(CmdTransfer::new(block.name, Arc::new(content)));
        }

        let core = self.cli.raw(command).await?;
        self.core_out(core);

        Ok(())
    }

    pub fn core_out(&self, core: ReflectedCore) {
        match core.is_ok() {
            true => self.out(core.body),
            false => {
                if core.body != Substance::Empty {
                    self.out(core.body);
                } else {
                    self.out_err(core.ok_or().unwrap_err());
                    std::process::exit(1);
                }
            }
        }
    }

    pub fn out(&self, substance: Substance) {
        match substance {
            Substance::Empty => {
                println!("Ok");
            }
            Substance::Err(err) => {
                println!("{}", err.to_string());
            }
            Substance::List(list) => {
                for i in list.list {
                    self.out(*i);
                }
            }
            Substance::Point(point) => {
                println!("{}", point.to_string());
            }
            Substance::Surface(surface) => {
                println!("{}", surface.to_string());
            }
            Substance::Text(text) => {
                println!("{}", text);
            }
            Substance::Stub(stub) => {
                println!("{}<{}>", stub.point.to_string(), stub.kind.to_string())
            }
            Substance::Details(details) => {
                println!(
                    "{}<{}>",
                    details.stub.point.to_string(),
                    details.stub.kind.to_string()
                )
            }
            what => {
                eprintln!(
                    "cosmic-cli not sure how to output {}",
                    what.kind().to_string()
                )
            }
        }
    }

    pub fn out_err(&self, err: SpaceErr) {
        eprintln!("{}", err.to_string())
    }
}

