#![allow(warnings)]

#[macro_use]
extern crate anyhow;

#[macro_use]
extern crate async_trait;

use std::collections::HashMap;
use std::future::Future;
use std::prelude::rust_2021::TryInto;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Error;
use futures::future::select_all;
use futures::{FutureExt, SinkExt};
use tokio::sync::mpsc::error::{SendError, SendTimeoutError, TryRecvError};
use tokio::sync::{broadcast, mpsc, oneshot};

use dashmap::{DashMap, DashSet};
use mesh_portal::version::latest;
use mesh_portal::version::latest::artifact::{Artifact, ArtifactRequest, ArtifactResponse};
use mesh_portal::version::latest::config::{Document, PointConfig, PortalConfig};
use mesh_portal::version::latest::entity::response;
use mesh_portal::version::latest::fail;
use mesh_portal::version::latest::frame::CloseReason;
use mesh_portal::version::latest::id::{Point, Port};
use mesh_portal::version::latest::log::{RootLogger, SpanLogger};
use mesh_portal::version::latest::messaging::{
    Agent, ReqProto, ReqShell, RespShell, Scope, SysMethod,
};
use mesh_portal::version::latest::particle::Stub;
use mesh_portal::version::latest::payload::Substance;
use mesh_portal::version::latest::sys::{Assign, Sys};
use mesh_portal_versions::error::MsgErr;
use mesh_portal_versions::version::v0_0_1::id::id::{Layer, ToPoint, ToPort};
use mesh_portal_versions::version::v0_0_1::wave::{
    AsyncRequestHandler, AsyncRouter, AsyncTransmitter, MethodKind, Requestable, RespCore,
    RespXtra, RootInCtx, Wave, WaveXtra,
};
use std::fmt::Debug;
use tokio::task::yield_now;

#[derive(Clone)]
pub enum PortalEvent {
    PortalAdded(Point),
    PortalRemoved(Point),
    ParticleAdded(Point),
    ParticleRemoved(Point),
}

#[derive(Clone, Eq, PartialEq, Hash)]
pub enum PortalStatus {
    None,
    Initializing,
    Ready,
    Panic(String),
}

#[derive(Debug, Clone)]
pub struct PortalInfo {
    pub portal_key: String,
}

pub struct Portal {
    pub info: PortalInfo,
    pub config: PortalConfig,
    outlet_tx: mpsc::Sender<WaveXtra>,
    pub logger: RootLogger,
    broadcast_tx: broadcast::Sender<PortalEvent>,
    point: Point,
    transmitter: Arc<dyn AsyncTransmitter>,
    assigned: Arc<DashSet<Point>>,
}

impl Portal {
    pub fn new(
        info: PortalInfo,
        config: PortalConfig,
        outlet_tx: mpsc::Sender<WaveXtra>,
        broadcast_tx: broadcast::Sender<PortalEvent>,
        logger: RootLogger,
        point: Point,
        transmitter: Arc<dyn AsyncTransmitter>,
    ) -> (Self, mpsc::Sender<WaveXtra>) {
        let (inlet_tx, mut inlet_rx): (mpsc::Sender<WaveXtra>, mpsc::Receiver<WaveXtra>) =
            mpsc::channel(1024);
        {
            let outlet_tx = outlet_tx.clone();
            let logger = logger.point(point.clone());
            let transmitter = transmitter.clone();
            tokio::spawn(async move {
                loop {
                    let logger = logger.clone();
                    match inlet_rx.recv().await {
                        Some(frame) => {
                            let span = frame.span();
                            match frame {
                                WaveXtra::Req(frame) => {
                                    let request = frame.request;
                                    let stub = request.as_stub();
                                    match tokio::time::timeout(
                                        Duration::from_secs(config.response_timeout),
                                        transmitter.req(request),
                                    )
                                    .await
                                    {
                                        Ok(response) => {
                                            let frame = RespXtra::new(response);
                                            let frame = WaveXtra::Resp(frame);
                                            outlet_tx.send(frame).await;
                                        }
                                        _ => {
                                            let response = stub.err(MsgErr::timeout());
                                            let frame = RespXtra::new(response);
                                            let frame = WaveXtra::Resp(frame);
                                            outlet_tx.send(frame).await;
                                        }
                                    }
                                }
                                WaveXtra::Resp(frame) => {
                                    let response = frame.response;
                                    let logger = logger.opt_span(frame.span);
                                    transmitter.route(Wave::Resp(response)).await;
                                }
                            }
                        }
                        None => {
                            break;
                        }
                    }
                }
            });
        }

        let assigned = Arc::new(DashSet::new());

        (
            Self {
                info,
                config,
                logger,
                outlet_tx,
                broadcast_tx,
                point,
                transmitter,
                assigned,
            },
            inlet_tx,
        )
    }

    pub async fn assign(&self, assign: Assign) -> Result<(), MsgErr> {
        let point = assign.details.stub.point.clone();
        self.assigned.insert(point.clone());
        let logger = self.logger.point(point.clone());
        let logger = logger.span();
        let stub = assign.details.stub.clone();
        let assign: Sys = assign.into();
        let assign: Substance = assign.into();
        let mut request = ReqProto::sys(Point::local_portal().clone().to_port(), SysMethod::Assign);
        request.body(assign);
        request.fill_from(point.clone().to_port().with_layer(Layer::Shell));
        let frame = request.to_frame(Some(logger.current_span().clone()));
        let frame = frame.into();
        self.outlet_tx.send(frame).await;

        self.broadcast_tx.send(PortalEvent::ParticleAdded(point));
        Ok(())
    }

    async fn send_request(&self, request: ReqShell) -> RespShell {
        let stub = request.as_stub();
        match tokio::time::timeout(
            Duration::from_secs(self.config.response_timeout),
            self.transmitter.req(request),
        )
        .await
        {
            Ok(Ok(response)) => response,
            _ => {
                let response = stub.err(MsgErr::timeout());
                response
            }
        }
    }

    pub fn shutdown(&mut self) {}

    pub fn has_core_port(&self, port: &Port) -> Result<(), ()> {
        if let Layer::Shell = port.layer {
            return Err(());
        }

        let point = port.clone().to_point();
        if self.point.is_parent(&point).is_ok() {
            return Ok(());
        }

        if self.assigned.contains(&point) {
            return Ok(());
        }

        Err(())
    }
}

#[async_trait]
impl AsyncRouter for Portal {
    async fn route(&self, wave: Wave) {
        match wave {
            Wave::Req(request) => {
                if self.has_core_port(&request.to).is_err() {
                    self.transmitter.route(request.not_found().into()).await;
                    return;
                }

                // don't allow anyone to say this request came from itself
                if self.has_core_port(&request.from).is_ok() {
                    self.transmitter.route(request.forbidden().into()).await;
                    return;
                }

                // the portal shell sends Sys messages...
                if request.core.method.kind() == MethodKind::Sys {
                    self.transmitter.route(request.forbidden().into()).await;
                    return;
                }
                let logger = self.logger.point(request.to.clone().to_point());
                let span = logger.span_async().current_span().clone();
                let frame = request.to_frame(Some(span));
                let frame = WaveXtra::Req(frame);
                self.outlet_tx.send(frame).await;
            }
            Wave::Resp(response) => {
                // Portal only handles requests (it should be expecting the response)
            }
        }
    }
}

#[cfg(test)]
pub mod test {

    #[test]
    pub fn test() {}
}
