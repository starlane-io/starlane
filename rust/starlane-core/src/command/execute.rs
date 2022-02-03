use mesh_portal_serde::version::latest::entity::request::create::Create;
use mesh_portal_serde::version::latest::entity::request::{Rc, RcCommand};
use mesh_portal_serde::version::latest::entity::request::select::Select;
use mesh_portal_serde::version::latest::messaging::{Request, Response};
use mesh_portal_serde::version::latest::payload::{Payload, Primitive};
use mesh_portal_serde::version::latest::resource::ResourceStub;
use mesh_portal_versions::version::v0_0_1::entity::request::create::Fulfillment;
use mesh_portal_versions::version::v0_0_1::entity::request::ReqEntity;
use mesh_portal_versions::version::v0_0_1::entity::response::RespEntity;
use mesh_portal_versions::version::v0_0_1::parse::Res;
use tokio::sync::mpsc;
use crate::command::cli::outlet;
use crate::command::compose::CommandOp;
use crate::command::parse::command_line;
use crate::star::StarSkel;
use crate::starlane::api::StarlaneApi;

pub struct CommandExecutor
{
    api: StarlaneApi,
    stub: ResourceStub,
    line: String,
    output_tx: mpsc::Sender<outlet::Frame>,
    fulfillments: Vec<Fulfillment>
}

impl CommandExecutor {

   pub async fn execute( line: String, output_tx: mpsc::Sender<outlet::Frame>, stub: ResourceStub, api: StarlaneApi, fulfillments: Vec<Fulfillment>) {
       let executor = Self {
           api,
           stub,
           output_tx,
           line,
           fulfillments
       };
       tokio::task::spawn_blocking(move || {
           tokio::spawn( async move {
               executor.parse().await;
           })
       }).await;
   }

    async fn parse( &self ) {
        match command_line(self.line.as_str() )
        {
            Ok((_,op)) => {
println!("PARSED...");
                match op {
                    CommandOp::Create(create) => {
                        self.exec_create(create).await;
                    }
                    CommandOp::Select(select) => {
                        self.exec_select(select).await;
                    }
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

println!("EXEC CREATE!");
        let parent = create.template.address.parent.clone();
        let entity = ReqEntity::Rc(Rc::new(RcCommand::Create(create)));
        let request = Request::new( entity, self.stub.address.clone(), parent );
        match self.api.exchange(request).await {
            Ok(response) => {
                match response.entity {
                    RespEntity::Ok(_) => {
                        self.output_tx.send(outlet::Frame::EndOfCommand(0)).await;
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

    async fn exec_select( &self, select: Select) {

println!("EXEC SELECT!");
        let query_root = select.pattern.query_root();
        let entity = ReqEntity::Rc(Rc::new(RcCommand::Select(select)) );
println!("QUERY ROOT {}", query_root.to_string() );
        let request = Request::new(entity, self.stub.address.clone(), query_root);
        match self.api.exchange(request).await {
            Ok(response) => {
println!("GOT SELECT RESPONSE!");
                match response.entity {
                    RespEntity::Ok(Payload::List(list)) => {
println!("stubs: {}",list.len());
                        for stub in list.iter() {
                            if let Primitive::Stub(stub) = stub {
                                self.output_tx.send(outlet::Frame::StdOut( stub.clone().address_and_kind().to_string() ) ).await;
                            }
                        }
println!("select all done.");
                        self.output_tx.send(outlet::Frame::EndOfCommand(0)).await;
                    }
                    RespEntity::Ok(_) => {
                        self.output_tx.send(outlet::Frame::StdErr( "unexpected response".to_string() ) ).await;
                        self.output_tx.send( outlet::Frame::EndOfCommand(1)).await;
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

