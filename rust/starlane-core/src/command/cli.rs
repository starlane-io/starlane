use std::convert::{TryFrom, TryInto};
use std::fmt::write;
use std::marker::PhantomData;
use mesh_portal::error;
use mesh_portal::version::latest::bin::Bin;
use mesh_portal::version::latest::entity::request::create::{AddressSegmentTemplate, KindTemplate, Template};
use mesh_portal::version::latest::frame::PrimitiveFrame;
use mesh_portal::version::latest::id::Address;
use mesh_portal::version::latest::messaging::Message;
use mesh_portal::version::latest::resource::ResourceStub;
use mesh_portal_tcp_common::{PrimitiveFrameReader, PrimitiveFrameWriter};
use mesh_portal::version::latest::entity::request::create::{AddressTemplate, Fulfillment};
use mesh_portal::version::latest::id::RouteSegment;
use mesh_portal_versions::version::v0_0_1::parse::Res;
use serde::Serialize;
use tokio::io::AsyncWriteExt;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::net::ToSocketAddrs;
use tokio::sync::mpsc;
use crate::command::cli::outlet::Frame;
use crate::command::execute::CommandExecutor;
use crate::command::parse::command_line;
use crate::error::Error;
use crate::star::shell::sys::SysResource;
use crate::star::StarSkel;
use crate::starlane::api::StarlaneApi;
use crate::starlane::{AuthRequestFrame, AuthResponseFrame, ServiceSelection, ServiceSelectionResponse};


pub mod inlet {
    use std::convert::{TryFrom, TryInto};
    use mesh_portal::version::latest::bin::Bin;
    use mesh_portal::version::latest::frame::PrimitiveFrame;
    use serde::{Serialize, Deserialize};
    use crate::error::Error;

    #[derive(Debug,Clone,Serialize,Deserialize)]
    pub enum Frame {
        CommandLine(String),
        TransferFile{ name: String, content: Bin },
        EndRequires
    }

    impl TryFrom<PrimitiveFrame> for Frame {
        type Error = mesh_portal::error::Error;

        fn try_from(value: PrimitiveFrame) -> Result<Self, Self::Error> {
            Ok(bincode::deserialize(value.data.as_slice() )?)
        }
    }

    impl TryInto<PrimitiveFrame> for Frame {
        type Error = Error;

        fn try_into(self) -> Result<PrimitiveFrame, Self::Error> {
            Ok(PrimitiveFrame { data: bincode::serialize(&self )? })
        }
    }
}

pub mod outlet{
    use std::convert::{TryFrom, TryInto};
    use mesh_portal::version::latest::frame::PrimitiveFrame;
    use serde::{Serialize, Deserialize};
    use crate::error::Error;

    #[derive(Debug,Clone,Serialize,Deserialize, strum_macros::Display)]
    pub enum Frame {
        StdOut(String),
        StdErr(String),
        EndOfCommand(i32)
    }

    impl TryFrom<PrimitiveFrame> for Frame {
        type Error = mesh_portal::error::Error;

        fn try_from(value: PrimitiveFrame) -> Result<Self, Self::Error> {
            Ok(bincode::deserialize(value.data.as_slice() )?)
        }
    }

    impl TryInto<PrimitiveFrame> for Frame {
        type Error = Error;

        fn try_into(self) -> Result<PrimitiveFrame, Self::Error> {
            Ok(PrimitiveFrame { data: bincode::serialize(&self )? })
        }
    }
}

pub struct CliServer {

}


impl CliServer {
    pub async fn new( api: StarlaneApi, mut reader: OwnedReadHalf, mut writer: OwnedWriteHalf) -> Result<(),Error> {
        let template = Template {
            address: AddressTemplate {
                parent: Address::root(),
                child_segment_template: AddressSegmentTemplate::Pattern("control-%".to_string())
            },
            kind: KindTemplate {
                resource_type: "Control".to_string(),
                kind: None,
                specific: None
            }
        };

        let (messenger_tx, mut messenger_rx) = mpsc::channel(1024);

        tokio::spawn(async move {
            while let Some(_) = messenger_rx.recv().await {
                // ignore messages for now
            }
        });


        let stub = api.create_sys_resource(template,messenger_tx).await?;

        let mut reader :FrameReader<inlet::Frame> = FrameReader::new( PrimitiveFrameReader::new( reader ));
        let mut writer: FrameWriter<outlet::Frame> = FrameWriter::new( PrimitiveFrameWriter::new( writer ));
        let (output_tx,mut output_rx):(mpsc::Sender<outlet::Frame>, mpsc::Receiver<outlet::Frame>) = mpsc::channel(1024);

        {
            let stub = stub.clone();
            let output_tx = output_tx.clone();
            tokio::task::spawn_blocking(move || {
                tokio::spawn(async move {

                    while let Ok(frame) = reader.read().await {
                        match frame {
                            inlet::Frame::CommandLine(line) => {
                                let mut fulfillments = vec![];

                                while let Ok(frame) = reader.read().await {
                                    match frame {
                                        inlet::Frame::TransferFile { name, content } => {
                                            fulfillments.push( Fulfillment::File {name,content});
                                        }
                                        inlet::Frame::EndRequires => {break;}
                                        _ => {
                                            eprintln!("cannot have this type of frame when sending requirements.");
                                            return;
                                        }
                                    }
                                }

                                CommandExecutor::execute(line, output_tx.clone(), stub.clone(), api.clone(), fulfillments ).await;
                            }
                            _ =>  {
                                eprintln!( "can only handle CommandLine frames until an executor has been selected");
                            }
                        }
                    }
                })
            });
        }


        {
            tokio::task::spawn_blocking(move || {
                tokio::spawn(async move {
                    while let Some(frame) = output_rx.recv().await {
                        let frame:outlet::Frame = frame;
                        writer.write(frame).await;
                    }
                })
            });
        }

        Ok(())
    }

    pub async fn new_internal( api: StarlaneApi ) -> Result<(mpsc::Sender<inlet::Frame>,mpsc::Receiver<outlet::Frame>),Error> {
        let template = Template {
            address: AddressTemplate {
                parent: Address::root(),
                child_segment_template: AddressSegmentTemplate::Pattern("control-%".to_string())
            },
            kind: KindTemplate {
                resource_type: "Control".to_string(),
                kind: None,
                specific: None
            }
        };

        let (messenger_tx, mut messenger_rx) = mpsc::channel(1024);

        tokio::spawn(async move {
            while let Some(_) = messenger_rx.recv().await {
                // ignore messages for now
            }
        });


        let stub = api.create_sys_resource(template,messenger_tx).await?;

        let (output_tx,mut output_rx):(mpsc::Sender<outlet::Frame>, mpsc::Receiver<outlet::Frame>) = mpsc::channel(1024);
        let (input_tx,mut input_rx):(mpsc::Sender<inlet::Frame>, mpsc::Receiver<inlet::Frame>) = mpsc::channel(1024);

        {
            let stub = stub.clone();
            let output_tx = output_tx.clone();
            tokio::task::spawn_blocking(move || {
                tokio::spawn(async move {

                    while let Some(frame) = input_rx.recv().await {
                        match frame {
                            inlet::Frame::CommandLine(line) => {
                                let mut fulfillments = vec![];

                                while let Some(frame) = input_rx.recv().await {
                                    match frame {
                                        inlet::Frame::TransferFile { name, content } => {
                                            fulfillments.push( Fulfillment::File {name,content});
                                        }
                                        inlet::Frame::EndRequires => {break;}
                                        _ => {
                                            eprintln!("cannot have this type of frame when sending requirements.");
                                            return;
                                        }
                                    }
                                }

                                CommandExecutor::execute(line, output_tx.clone(), stub.clone(), api.clone(), fulfillments ).await;
                            }
                            _ =>  {
                                eprintln!( "can only handle CommandLine frames until an executor has been selected");
                            }
                        }
                    }
                })
            });
        }

        Ok((input_tx,output_rx))
    }
}

pub struct CliClient {
    reader: FrameReader<outlet::Frame>,
    writer: FrameWriter<inlet::Frame>
}

impl CliClient {

    pub async fn new( host: String, token: String ) -> Result<Self,Error> {
        let mut stream =
            match TcpStream::connect(host.clone()).await {
                Ok(stream) => stream,
                Err(err) => {
                    return Err(format!("could not connect to: '{}' because: {}", host, err.to_string()).into());
                }
            };
        let (reader,writer) = stream.into_split();
        let mut reader : FrameReader<AuthResponseFrame> = FrameReader::new( PrimitiveFrameReader::new( reader ));
        let mut writer : FrameWriter<AuthRequestFrame>  = FrameWriter::new( PrimitiveFrameWriter::new( writer ));

        // first send token
        writer.write(AuthRequestFrame::Token(token)).await?;

        let response = reader.read().await?;

        let mut reader = reader.done().done();
        let mut writer = writer.done().done();

        let mut reader : FrameReader<ServiceSelectionResponse> = FrameReader::new( PrimitiveFrameReader::new( reader ));
        let mut writer : FrameWriter<ServiceSelection>  = FrameWriter::new( PrimitiveFrameWriter::new( writer ));

        writer.write(ServiceSelection::Cli ).await?;
        reader.read().await?;

        let mut reader : FrameReader<outlet::Frame> = FrameReader::new( reader.done() );
        let mut writer : FrameWriter<inlet::Frame>  = FrameWriter::new( writer.done() );


        Ok(Self {
            reader,
            writer
        })
    }

    pub async fn send( mut self, command_line: String ) -> Result<CommandExchange,Error> {
        let exchange = tokio::task::spawn_blocking( move || {
            tokio::spawn(async move {
                self.writer.write( inlet::Frame::CommandLine(command_line)).await;
                self.into()
            } )
        }).await?.await?;

        Ok(exchange)
    }


}

impl Into<CommandExchange> for CliClient {
    fn into(self) -> CommandExchange{
        CommandExchange {
            reader: self.reader,
            writer: self.writer,
            complete: false
        }
    }
}

impl Into<CliClient> for CommandExchange{
    fn into(self) -> CliClient{
        CliClient{
            reader: self.reader,
            writer: self.writer
        }
    }
}


pub struct CommandExchange {
    reader: FrameReader<outlet::Frame>,
    writer: FrameWriter<inlet::Frame>,
    complete: bool
}

impl CommandExchange {

    pub async fn file( &mut self, name: String, content: Bin ) -> Result<(),Error> {
        self.write( inlet::Frame::TransferFile{name,content}).await
    }

    pub async fn end_requires( &mut self ) -> Result<(),Error> {
        self.write( inlet::Frame::EndRequires ).await
    }

    pub async fn read( &mut self ) -> Option<Result<outlet::Frame,Error>> {
        if self.complete {
            return Option::None;
        }

        async fn handle( exchange: &mut CommandExchange ) -> Result<outlet::Frame,Error> {
            let frame = exchange.reader.read().await?;

            if let outlet::Frame::EndOfCommand(code) = frame {
                exchange.complete = true;
            }
            Ok(frame)
        }

        Option::Some(handle(self).await)
    }

    pub async fn write( &mut self, frame: inlet::Frame ) -> Result<(),Error> {
       self.writer.write(frame).await?;
       Ok(())
    }
}

pub enum Output {
    StdOut(String),
    StdErr(String),
    End(i32)
}




pub struct FrameWriter<FRAME> where FRAME: Serialize {
    stream: PrimitiveFrameWriter,
    phantom: PhantomData<FRAME>
}

impl <FRAME> FrameWriter<FRAME> where FRAME: Serialize {
    pub fn new(stream: PrimitiveFrameWriter) -> Self {
        Self {
            stream,
            phantom: PhantomData
        }
    }

    pub fn done(self) -> PrimitiveFrameWriter {
        self.stream
    }
}

impl <F> FrameWriter<F> where F: Serialize  {

    pub async fn write( &mut self, frame: F ) -> Result<(),Error> {
        bincode::serialize(&frame)?;
        Ok(())
    }

}

/*

impl FrameWriter<outlet::Frame>  {

    pub async fn write( &mut self, frame: outlet::Frame ) -> Result<(),Error> {
        let frame = frame.try_into()?;
        Ok(self.stream.write(frame).await?)
    }

}

impl FrameWriter<inlet::Frame> {

    pub async fn write( &mut self, frame: inlet::Frame ) -> Result<(),Error> {
        let frame = frame.try_into()?;
        Ok(self.stream.write(frame).await?)
    }
}

 */


pub struct FrameReader<FRAME> {
    stream: PrimitiveFrameReader,
    phantom: PhantomData<FRAME>
}

impl <FRAME> FrameReader<FRAME>  {
    pub fn new(stream: PrimitiveFrameReader) -> Self {
        Self {
            stream,
            phantom: PhantomData
        }
    }
}

impl <Frame> FrameReader<Frame> where Frame: TryFrom<PrimitiveFrame,Error=error::Error> {
    pub async fn read( &mut self ) -> Result<Frame,Error> {
        let frame = self.stream.read().await?;
        Ok(Frame::try_from(frame)?)
    }
}

impl <F> FrameReader<F> {
    pub fn done(self) -> PrimitiveFrameReader {
        self.stream
    }
}

/*
impl FrameReader<outlet::Frame> {
    pub async fn read( &mut self ) -> Result<outlet::Frame,Error> {
        let frame = self.stream.read().await?;
        Ok(outlet::Frame::try_from(frame)?)
    }
}

impl FrameReader<inlet::Frame> {
    pub async fn read( &mut self ) -> Result<inlet::Frame,Error> {
        let frame = self.stream.read().await?;
        Ok(inlet::Frame::try_from(frame)?)
    }
}

 */