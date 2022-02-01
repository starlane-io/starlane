use mesh_portal_serde::version::latest::entity::request::create::Create;
use mesh_portal_serde::version::latest::entity::request::{Rc, RcCommand};
use mesh_portal_serde::version::latest::messaging::{Request, Response};
use mesh_portal_serde::version::latest::resource::ResourceStub;
use mesh_portal_versions::version::v0_0_1::entity::request::ReqEntity;
use mesh_portal_versions::version::v0_0_1::entity::response::RespEntity;
use mesh_portal_versions::version::v0_0_1::parse::Res;
use tokio::sync::mpsc;
use crate::command::cli::outlet;
use crate::command::compose::CommandOp;
use crate::command::parse::command_line;
use crate::star::StarSkel;

pub struct CommandExecutor
{
    skel: StarSkel,
    stub: ResourceStub,
    line: String,
    output_tx: mpsc::Sender<outlet::Frame>
}

impl CommandExecutor {
   pub async fn execute( line: String, output_tx: mpsc::Sender<outlet::Frame>, stub: ResourceStub, skel: StarSkel ) {
       let executor = Self {
           skel,
           stub,
           output_tx,
           line
       };
       executor.parse();
   }

    async fn parse( &self ) {
        match command_line(self.line.as_str() )
        {
            Ok((_,op)) => {
                match op {
                    CommandOp::Create(create) => {
                        self.exec_create(create);
                    }
                    CommandOp::Select(select) => {}
                    CommandOp::Publish(create_op) => {}
                }
            }
            Err(err) => {
                self.output_tx.send(outlet::Frame::StdErr( err.to_string() ) ).await;
                self.output_tx.send( outlet::Frame::EndOfCommand(1)).await;
                return;
            }
        }
    }

    async fn exec_create( &self, create: Create  ) {
        let parent = create.template.address.parent.clone();
        let entity = ReqEntity::Rc(Rc::new(RcCommand::Create(create)));
        let request = Request::new( entity, self.stub.address.clone(), parent );
        match self.skel.messaging_api.exchange(request).await {
            Ok(response) => {
                match response.entity {
                    RespEntity::Ok(_) => {
                        self.output_tx.send( outlet::Frame::EndOfCommand(0)).await;
                    }
                    RespEntity::Fail(fail) => {
                        self.output_tx.send(outlet::Frame::StdErr( fail.to_string() ) ).await;
                        self.output_tx.send( outlet::Frame::EndOfCommand(1)).await;
                    }
                }
            }
            Err(err) => {
                self.output_tx.send(outlet::Frame::StdErr( err.to_string() ) ).await;
                self.output_tx.send( outlet::Frame::EndOfCommand(1)).await;
            }
        }
    }
}

