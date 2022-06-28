#![allow(warnings)]
#[macro_use]
extern crate async_trait;

#[macro_use]
extern crate anyhow;

#[macro_use]
extern crate cosmic_macros;

#[macro_use]
extern crate cosmic_macros_primitive;

#[macro_use]
extern crate lazy_static;

use dashmap::{DashMap, DashSet};
use mesh_portal::error::MsgErr;
use mesh_portal::version::latest::entity::response::RespCore;
use mesh_portal::version::latest::id::{Point, Port, Uuid};
use mesh_portal::version::latest::messaging::{Agent, ReqCtx, SysMethod};
use mesh_portal::version::latest::payload::Substance;
use mesh_portal::version::latest::sys::{Assign, Sys};
use mesh_portal_versions::version::v0_0_1::id::id::{Layer, ToPoint, ToPort};
use mesh_portal_versions::version::v0_0_1::quota::Timeouts;
use mesh_portal_versions::version::v0_0_1::wave::{
    AsyncInternalRequestHandlers, AsyncPointRequestHandlers, AsyncRequestHandler,
    AsyncRequestHandlerRelay, AsyncRouter, AsyncTransmitter, ProtoTransmitter, ReqShell, ReqXtra,
    RequestHandlerRelay, Requestable, RespShell, RespXtra, WaitTime, Wave, WaveXtra,
};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

pub struct PortalCore {
    pub port: Port,
    pub assigned: Arc<DashSet<Point>>,
    pub transmitter: Arc<dyn AsyncTransmitter>,
    pub handlers: AsyncInternalRequestHandlers<AsyncRequestHandlerRelay>,
}

impl PortalCore {
    pub async fn new(
        inlet_tx: mpsc::Sender<Wave>,
        outlet_rx: mpsc::Receiver<Wave>,
    ) -> Result<Self, MsgErr> {
        let assigned = Arc::new(DashSet::new());
        let messenger = Arc::new(PortalTransmitter::new(inlet_tx, assigned.clone()));

        async fn listen_for_point(
            mut outlet_rx: mpsc::Receiver<Wave>,
        ) -> Result<(Point, mpsc::Receiver<Wave>), MsgErr> {
            while let Ok(frame) = tokio::time::timeout(
                Duration::from_secs(Timeouts::default().from(WaitTime::High)),
                outlet_rx.recv(),
            )
            .await
            {
                if let Some(frame) = frame {
                    if let Wave::Req(request) = frame {
                        let point: Point = request
                            .require_method(SysMethod::AssignPort)?
                            .core
                            .body
                            .try_into()?;
                        return Ok((point, outlet_rx));
                    } // else do nothing...
                } else {
                    return Err(MsgErr::server_error());
                }
            }
            Err(MsgErr::timeout())
        }

        let (point, mut outlet_rx) = listen_for_point(outlet_rx).await?;
        assigned.insert(point.clone());

        let handlers = AsyncInternalRequestHandlers::new();

        let mut port = point.to_port().with_layer(Layer::Core);

        {
            let handlers = handlers.clone();
            tokio::spawn(async move {
                while let Some(wave) = outlet_rx.recv().await {
                    // process wave somehow...
                }
            });
        }

        Ok(Self {
            port,
            assigned,
            transmitter: messenger,
            handlers,
        })
    }

    pub fn has_core(&self, point: &Point) -> Result<(), ()> {
        if self.port.point == *point {
            return Ok(());
        }
        if self.assigned.contains(point) {
            Ok(())
        } else {
            Err(())
        }
    }
}

pub struct PortalTransmitter {
    inlet_tx: mpsc::Sender<Wave>,
    exchanges: Arc<DashMap<Uuid, oneshot::Sender<RespShell>>>,
    assigned: Arc<DashSet<Point>>,
    timeouts: Timeouts,
}

impl PortalTransmitter {
    pub fn new(inlet_tx: mpsc::Sender<Wave>, assigned: Arc<DashSet<Point>>) -> Self {
        Self {
            inlet_tx,
            exchanges: Arc::new(DashMap::new()),
            assigned,
            timeouts: Default::default(),
        }
    }
}

#[async_trait]
impl AsyncTransmitter for PortalTransmitter {
    async fn req(&self, request: ReqShell) -> RespShell {
        if !self.assigned.contains(&request.from.clone().to_point()) {
            return request.forbidden();
        }

        let (tx, rx) = oneshot::channel();
        self.exchanges.insert(request.id.clone(), tx);
        let stub = request.as_stub();
        self.inlet_tx.send(request.into()).await;

        if let Ok(frame) =
            tokio::time::timeout(Duration::from_secs(self.timeouts.from(&stub)), rx).await
        {
            if let Ok(response) = frame {
                response
            } else {
                stub.server_error()
            }
        } else {
            stub.timeout()
        }
    }

    async fn route(&self, wave: Wave) {
        unimplemented!()
    }
}

pub trait PortalCtrlFactory: Send + Sync {
    fn create(
        &self,
        assign: Assign,
        tx: ProtoTransmitter,
    ) -> Result<Box<dyn AsyncRequestHandler>, MsgErr>;
}

#[derive(AsyncRequestHandler)]
pub struct PortalRequestHandler {
    factory: Box<dyn PortalCtrlFactory>,
    handlers: AsyncPointRequestHandlers,
    messenger: Arc<dyn AsyncTransmitter>,
}

#[routes_async(self.handlers)]
impl PortalRequestHandler {
    pub fn new(
        transmitter: Arc<dyn AsyncTransmitter>,
        factory: Box<dyn PortalCtrlFactory>,
    ) -> Self {
        Self {
            factory,
            handlers: AsyncPointRequestHandlers::new(),
            messenger: transmitter,
        }
    }

    #[route("Sys<Assign>")]
    pub async fn assign(&self, request: ReqCtx<'_, Assign>) -> Result<RespCore, MsgErr> {
        let mut transmitter = ProtoTransmitter::new(self.messenger.clone());

        self.handlers.add(
            request.details.stub.point.clone(),
            self.factory.create(request.input.clone(), transmitter)?,
        );
        Ok(RespCore::ok(Substance::Empty))
    }
}
