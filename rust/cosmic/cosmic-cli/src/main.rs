#[macro_use]
extern crate clap;
use clap::{App, Arg, Args, Command, Parser, Subcommand};
use clap::arg;
use clap::command;



fn main() {

    let matches = Command::new("comsic-cli").arg( Arg::new("host").short('h').long("host").takes_value(true).value_name("host").required(false).default_value("localhost")).allow_external_subcommands(true).get_matches();

        println!(" HOST : {}", matches.get_one::<String>("host").unwrap());


    if matches.subcommand_name().is_some() {
        println!("SUBCOMMAND: {}",matches.subcommand_name().unwrap());
    }

}
