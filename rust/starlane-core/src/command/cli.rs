use std::convert::{TryFrom, TryInto};
use std::marker::PhantomData;
use mesh_portal_serde::version::latest::entity::request::create::{AddressSegmentTemplate, KindTemplate, Template};
use mesh_portal_serde::version::latest::frame::PrimitiveFrame;
use mesh_portal_serde::version::latest::id::Address;
use mesh_portal_serde::version::latest::messaging::Message;
use mesh_portal_serde::version::latest::resource::ResourceStub;
use mesh_portal_tcp_common::{PrimitiveFrameReader, PrimitiveFrameWriter};
use mesh_portal_versions::version::v0_0_1::entity::request::create::AddressTemplate;
use mesh_portal_versions::version::v0_0_1::id::RouteSegment;
use mesh_portal_versions::version::v0_0_1::parse::Res;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use crate::command::cli::outlet::Frame;
use crate::command::execute::CommandExecutor;
use crate::command::parse::command_line;
use crate::error::Error;
use crate::star::shell::sys::SysResource;
use crate::star::StarSkel;


pub mod inlet {
    use std::convert::TryFrom;
    use mesh_portal_serde::version::latest::frame::PrimitiveFrame;
    use serde::{Serialize, Deserialize};
    use crate::error::Error;

    #[derive(Debug,Clone,Serialize,Deserialize)]
    pub enum Frame {
        CommandLine(String)
    }

    impl TryFrom<PrimitiveFrame> for Frame {
        type Error = Error;

        fn try_from(value: PrimitiveFrame) -> Result<Self, Self::Error> {
            Ok(bincode::deserialize(value.data.as_slice() )?)
        }
    }
}

pub mod outlet{
    use std::convert::TryFrom;
    use mesh_portal_serde::version::latest::frame::PrimitiveFrame;
    use serde::{Serialize, Deserialize};
    use crate::error::Error;

    #[derive(Debug,Clone,Serialize,Deserialize)]
    pub enum Frame {
        StdOut(String),
        StdErr(String),
        EndOfCommand(i32)
    }

    impl TryFrom<PrimitiveFrame> for Frame {
        type Error = Error;

        fn try_from(value: PrimitiveFrame) -> Result<Self, Self::Error> {
            Ok(bincode::deserialize(value.data.as_slice() )?)
        }
    }
}

pub struct CliService {

}


impl CliService {
    pub async fn new( skel: StarSkel, mut stream: TcpStream ) -> Result<(),Error> {
        let template = Template {
            address: AddressTemplate {
                parent: Address::root_with_route(RouteSegment::Mesh(skel.info.key.to_string())),
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


        let stub = skel.sys_api.create(template,messenger_tx).await?;

        let (reader,writer) = stream.into_split();

        let mut reader :FrameReader<inlet::Frame> = FrameReader::new( PrimitiveFrameReader::new( reader ));
        let mut writer = FrameWriter::new( PrimitiveFrameWriter::new( writer ));
        let (output_tx,mut output_rx) = mpsc::channel(1024);

        {
            let skel = skel.clone();
            let stub = stub.clone();
            tokio::task::spawn_blocking(move || {
                tokio::spawn(async move {
                    while let Ok(frame) = reader.read().await {
                        match frame {
                            inlet::Frame::CommandLine(line) => {
                                CommandExecutor::execute(line, output_tx.clone(), stub.clone(), skel.clone() ).await;
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
                        writer.write(frame).await;
                    }
                })
            });
        }

        Ok(())
    }


}
pub struct FrameWriter<FRAME> where FRAME: TryInto<PrimitiveFrame> {
    stream: PrimitiveFrameWriter,
    phantom: PhantomData<FRAME>
}

impl <FRAME> FrameWriter<FRAME> where FRAME: TryInto<PrimitiveFrame>  {
    pub fn new(stream: PrimitiveFrameWriter) -> Self {
        Self {
            stream,
            phantom: PhantomData
        }
    }
}

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