use crate::command::cli::outlet;
use crate::command::compose::CommandOp;
use crate::command::parse::command_line;
use crate::particle::KindBase;
use crate::star::StarSkel;
use crate::starlane::api::StarlaneApi;
use mesh_portal::version::latest::entity::request::create::{Create, CreateOp, Set};
use mesh_portal::version::latest::entity::request::get::Get;
use mesh_portal::version::latest::entity::request::select::Select;
use mesh_portal::version::latest::entity::request::{Method, Rc};
use mesh_portal::version::latest::messaging::{Request, Response};
use mesh_portal::version::latest::payload::{Payload, Primitive};
use mesh_portal::version::latest::particle::Stub;
use mesh_portal::error::MsgErr;
use mesh_portal::version::latest::entity::request::create::{Fulfillment, KindTemplate};
use mesh_portal_versions::version::v0_0_1::parse::{Env, Res, ToResolved};
use std::string::FromUtf8Error;
use mesh_portal_versions::version::v0_0_1::span::new_span;
use tokio::sync::mpsc;

pub struct CommandExecutor {
    api: StarlaneApi,
    stub: Stub,
    line: String,
    output_tx: mpsc::Sender<outlet::Frame>,
    fulfillments: Vec<Fulfillment>,
    env: Env
}

impl CommandExecutor {
    pub fn exec_simple(
        line: String,
        stub: Stub,
        api: StarlaneApi,
    ) -> mpsc::Receiver<outlet::Frame> {
        let (output_tx, output_rx) = mpsc::channel(1024);
        tokio::spawn(async move {
            CommandExecutor::execute(line, output_tx, stub, api, vec![]);
        });
        output_rx
    }

    pub async fn execute(
        line: String,
        output_tx: mpsc::Sender<outlet::Frame>,
        stub: Stub,
        api: StarlaneApi,
        fulfillments: Vec<Fulfillment>,
    ) {
        let executor = Self {
            api,
            stub,
            output_tx,
            line,
            fulfillments,
            env: Env::empty()
        };
        tokio::task::spawn_blocking(move || {
            tokio::spawn(async move {
                executor.parse().await;
            })
        })
        .await;
    }

    async fn parse(&self) {
        match command_line(new_span(self.line.as_str())) {
            Ok((_, op)) => match op {
                CommandOp::Create(create) => {
                    self.exec_create(create).await;
                }
                CommandOp::Select(select) => {
                    self.exec_select(select).await;
                }
                CommandOp::Publish(create_op) => {
                    let create_op = create_op.collapse().unwrap().collapse().unwrap();
                    self.exec_publish(create_op).await;
                }
                CommandOp::Set(set) => {
                    let set = set.collapse().unwrap();
                    self.exec_set(set).await;
                }
                CommandOp::Get(get) => {
                    let get = get.collapse().unwrap();
                    self.exec_get(get).await;
                }
            },
            Err(err) => {
                self.output_tx
                    .send(outlet::Frame::StdErr(err.to_string()))
                    .await;
                self.output_tx.send(outlet::Frame::EndOfCommand(1)).await;
                return;
            }
        }
    }

    async fn exec_create(&self, create: Create) {

        let parent = create.template.point.parent.clone();
        let action = Method::Cmd(Rc::Create(create));
        let request = Request::new(action.into(), self.stub.point.clone(), parent);
        let response = self.api.exchange(request).await;

        match response.ok_or() {
            Ok(_) => {
                self.output_tx.send(outlet::Frame::EndOfCommand(0)).await;
            }
            Err(err) => {
                self.output_tx
                    .send(outlet::Frame::StdErr(err.to_string()))
                    .await;
                self.output_tx.send(outlet::Frame::EndOfCommand(1)).await;
            }
        }
    }

    async fn exec_select(&self, select: Select) {
        let query_root = select.pattern.query_root();
        let action = Method::Cmd(Rc::Select(select));
        let core = action.into();
        let request = Request::new(core, self.stub.point.clone(), query_root);
        let response = self.api.exchange(request).await;
        match response.ok_or() {
            Ok(response) => {
                let response = match response.ok_or() {
                    Ok(response) => response,
                    Err(fail) => {
                        self.output_tx
                            .send(outlet::Frame::StdErr(fail.to_string()))
                            .await;
                        self.output_tx.send(outlet::Frame::EndOfCommand(1)).await;
                        return;
                    }
                };
                match &response.core.body {
                    Payload::List(list) => {
                        for stub in list.iter() {
                            if let Payload::Stub(stub) = &**stub {
                                self.output_tx
                                    .send(outlet::Frame::StdOut(
                                        stub.clone().point_and_kind().to_string(),
                                    ))
                                    .await;
                            }
                        }
                        self.output_tx.send(outlet::Frame::EndOfCommand(0)).await;
                    }
                    _ => {
                        self.output_tx
                            .send(outlet::Frame::StdErr("unexpected response".to_string()))
                            .await;
                        self.output_tx.send(outlet::Frame::EndOfCommand(1)).await;
                    }
                }
            }
            Err(err) => {
                self.output_tx
                    .send(outlet::Frame::StdErr(err.to_string()))
                    .await;
                self.output_tx.send(outlet::Frame::EndOfCommand(1)).await;
            }
        }
    }

    async fn exec_publish(&self, mut create: CreateOp) {
        if self.fulfillments.len() != 1 {
            self.output_tx
                .send(outlet::Frame::StdErr(
                    "Expected one and only one TransferFile fulfillment for publish".to_string(),
                ))
                .await;
            self.output_tx.send(outlet::Frame::EndOfCommand(1)).await;
            return;
        }
        if let Option::Some(Fulfillment::File { name, content }) = self.fulfillments.get(0).cloned()
        {
            let mut create = create.fulfillment(content);
            create.template.kind = KindTemplate {
                kind: KindBase::ArtifactBundle.to_string(),
                sub_kind: None,
                specific: None,
            };

            let parent = create.template.point.parent.clone();
            let action = Method::Cmd(Rc::Create(create));
            let core = action.into();
            let request = Request::new(core, self.stub.point.clone(), parent);
            match self.api.exchange(request).await.ok_or() {
                Ok(response) => match response.ok_or() {
                    Ok(_) => {
                        self.output_tx.send(outlet::Frame::EndOfCommand(0)).await;
                    }
                    Err(fail) => {
                        self.output_tx
                            .send(outlet::Frame::StdErr(fail.to_string()))
                            .await;
                        self.output_tx.send(outlet::Frame::EndOfCommand(1)).await;
                    }
                },
                Err(err) => {
                    self.output_tx
                        .send(outlet::Frame::StdErr(err.to_string()))
                        .await;
                    self.output_tx.send(outlet::Frame::EndOfCommand(1)).await;
                }
            }
        } else {
            self.output_tx
                .send(outlet::Frame::StdErr(
                    "Expected TransferFile fulfillment for publish".to_string(),
                ))
                .await;
            self.output_tx.send(outlet::Frame::EndOfCommand(1)).await;
            return;
        }
    }

    async fn exec_set(&self, set: Set) {
        let to = set.point.parent().clone().expect("expect parent");
        let action = Method::Cmd(Rc::Set(set));
        let core = action.into();
        let request = Request::new(core, self.stub.point.clone(), to);
        match self.api.exchange(request).await.ok_or() {
            Ok(response) => {
                    self.output_tx.send(outlet::Frame::EndOfCommand(0)).await;
            },
            Err(err) => {
                self.output_tx
                    .send(outlet::Frame::StdErr(err.to_string()))
                    .await;
                self.output_tx.send(outlet::Frame::EndOfCommand(1)).await;
            }
        }
    }

    async fn exec_get(&self, get: Get) {
        let to = get.point.parent().clone().expect("expect parent");
        let action = Method::Cmd(Rc::Get(get.clone()));
        let core = action.into();
        let request = Request::new(core, self.stub.point.clone(), to);
        match self.api.exchange(request).await.ok_or() {
            Ok(response) => match response.core.body {
                Payload::Bin(bin) => {
                    match String::from_utf8((*bin).clone()) {
                        Ok(text) => {
                            self.output_tx.send(outlet::Frame::StdOut(text)).await;
                            self.output_tx.send(outlet::Frame::EndOfCommand(0)).await;
                        }
                        Err(err) => {
                            self.output_tx
                                .send(outlet::Frame::StdErr(
                                    "Bin File Cannot be displayed on console".to_string(),
                                ))
                                .await;
                            self.output_tx.send(outlet::Frame::EndOfCommand(1)).await;
                        }
                    }
                }
                Payload::Text(text) => {
                    self.output_tx.send(outlet::Frame::StdOut(text)).await;
                    self.output_tx.send(outlet::Frame::EndOfCommand(0)).await;
                }
                Payload::Map(map) => {
                    let mut rtn = String::new();
                    rtn.push_str(get.point.to_string().as_str());
                    rtn.push_str("{ ");
                    for (index, (key, payload)) in map.iter().enumerate() {
                        if let Payload::Text(value) = payload {
                            rtn.push_str(key.as_str());
                            rtn.push_str("=");
                            rtn.push_str(value.as_str());
                            if index != map.len() - 1 {
                                rtn.push_str(", ");
                            }
                        }
                    }
                    rtn.push_str(" }");
                    self.output_tx.send(outlet::Frame::StdOut(rtn)).await;
                    self.output_tx.send(outlet::Frame::EndOfCommand(0)).await;
                }

                _ => {
                    self.output_tx
                        .send(outlet::Frame::StdErr(
                            "unexpected payload response format".to_string(),
                        ))
                        .await;
                    self.output_tx.send(outlet::Frame::EndOfCommand(1)).await;
                }
            },
            Err(err) => {
                self.output_tx
                    .send(outlet::Frame::StdErr(err.to_string()))
                    .await;
                self.output_tx.send(outlet::Frame::EndOfCommand(1)).await;
            }
        }
    }
}
