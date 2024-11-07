use crate::driver::control::{ControlCliSession, ControlClient};
use crate::hyperlane::tcp::HyperlaneTcpClient;
use crate::hyperlane::HyperwayEndpointFactory;
use clap::clap_derive::{Args, Subcommand};
use clap::Parser;
use starlane_space::space::parse::util::new_span;
use starlane::space::command::{CmdTransfer, RawCommand};
use starlane::space::err::SpaceErr;
use starlane::space::hyper::Knock;
use starlane::space::parse::{upload_blocks, SkewerCase};
use starlane::space::point::Point;
use starlane::space::substance::Substance;
use starlane::space::wave::core::ReflectedCore;
use std::fs::File;
use std::io::{Cursor, Read, Seek, Write};
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;
use strum_macros::EnumString;
use tokio::io::AsyncWriteExt;
use walkdir::{DirEntry, WalkDir};
use zip::write::FileOptions;
use starlane::space::parse::util::result;
use starlane_primitive_macros::logger;
use crate::env::STARLANE_HOME;

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[arg(
    short,
            long,
            default_value_t = true
    )]
    pub logs: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand, EnumString, strum_macros::Display)]
#[command(version, about, long_about = None)]
pub enum Commands {
    Install{
        #[arg(long,short)]
        edit: bool,
        #[arg(long,short)]
        nuke: bool
    },
    Run,
    Term(TermArgs),
    Version,
    Splash,
    Scorch,
    Nuke{
        #[arg(long)]
        all: bool
    },
    Context(ContextArgs)
}

#[derive(Debug,Args)]
pub struct ContextArgs{
    #[clap(subcommand)]
    pub command: ContextCmd,
}

impl Default for ContextArgs {
    fn default() -> Self {
        todo!()
    }
}

#[derive(Debug,Subcommand,EnumString, strum_macros::Display)]
pub enum ContextCmd {
    Create{ context_name: String},
    Switch{ context_name: String},
    Default,
    List,
    Which
}

#[derive(Debug, Args)]
pub struct TermArgs {
    #[arg(long)]
    host: Option<String>,

    /// Number of times to greet
    #[arg(long)]
    certs: Option<String>,

    #[arg(long)]
    history_log: Option<String>
}

impl Default for TermArgs {
    fn default() -> Self {

        Self {
            host: None,
            certs: None,
            history_log: None
        }
    }
}

pub async fn term(args: TermArgs) -> Result<(), SpaceErr> {
    let history_log = match args.history_log {
        None => format!("{}/history.log", STARLANE_HOME.to_string()).to_string(),
        Some(history) => history.to_string(),
    };

    let certs = match args.certs.as_ref() {
        None => format!("{}/localhost/certs", STARLANE_HOME.to_string()),
        Some(certs) => certs.clone(),
    };

    let host = match args.host.as_ref() {
        None => "localhost".to_string(),
        Some(host) => host.clone(),
    };


    let session = Session::new(host, certs).await?;

    let mut rl = rustyline::DefaultEditor::new().unwrap();
    rl.add_history_entry(history_log.as_str());
    rl.save_history(history_log.as_str());

    loop {

        let line = rl.readline(">> ").unwrap();
        rl.add_history_entry(history_log.as_str());

        let line_str = line.trim();

        if "exit" == line_str {
            return Ok(());
        }

        if line_str.len() > 0 {
            session.command(line.as_str()).await?;
        }
    }

}

pub struct Session {
    pub client: ControlClient,
    pub cli: ControlCliSession,
}

impl Session {
    pub async fn new(host: String, certs: String) -> Result<Self, SpaceErr> {
        let logger = logger!(Point::from_str("starlane-cli")?);
        let tcp_client: Box<dyn HyperwayEndpointFactory> = Box::new(HyperlaneTcpClient::new(
            format!("{}:{}", host, 4343),
            certs,
            Knock::default(),
            false,
            logger,
        ));

        let client = ControlClient::new(tcp_client)?;

        client.wait_for_ready(Duration::from_secs(30)).await?;
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

                let data = match zip_dir(
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
                .push(CmdTransfer::new(block.name, content));
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

fn zip_dir<T>(
    it: impl Iterator<Item = DirEntry>,
    prefix: &str,
    writer: T,
    method: zip::CompressionMethod,
) -> zip::result::ZipResult<T>
where
    T: Write + Seek,
{
    let mut zip = zip::ZipWriter::new(writer);
    let options = FileOptions::default()
        .compression_method(method)
        .unix_permissions(0o755);

    let mut buffer = Vec::new();
    for entry in it {
        let path = entry.path();
        let name = path.strip_prefix(Path::new(prefix)).unwrap();

        // Write file or directory explicitly
        // Some unzip tools unzip files with directory paths correctly, some do not!
        if path.is_file() {
            zip.start_file(name.to_str().unwrap(), options)?;
            let mut f = File::open(path)?;

            f.read_to_end(&mut buffer)?;
            zip.write_all(&*buffer)?;
            buffer.clear();
        } else if !name.as_os_str().is_empty() {
            // Only if not root! Avoids path spec / warning
            // and mapname conversion failed error on unzip
            zip.add_directory(name.to_str().unwrap(), options)?;
        }
    }
    let result = zip.finish()?;
    Result::Ok(result)
}
