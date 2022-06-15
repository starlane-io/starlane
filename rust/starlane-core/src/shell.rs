use std::sync::Arc;
use std::time::Duration;
use dashmap::DashMap;
use futures::SinkExt;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use cosmic_nom::Res;
use cosmic_portal_cli::Cli;
use cosmic_portal_cli_exe::CliRelay;
use mesh_portal::error::MsgErr;
use mesh_portal::version::latest::entity::response::ResponseCore;
use mesh_portal::version::latest::id::{Point, Port, Uuid};
use mesh_portal::version::latest::log::{LogSpan, PointLogger, RootLogger};
use mesh_portal::version::latest::messaging::{Agent, Request, Response, RootRequestCtx};
use mesh_portal_versions::version::v0_0_1::config::config::bind::RouteSelector;
use mesh_portal_versions::version::v0_0_1::id::id::{Layer, ToPoint, ToPort};
use mesh_portal_versions::version::v0_0_1::wave::{AsyncInternalRequestHandlers, AsyncMessenger, AsyncMessengerAgent, AsyncPointRequestHandlers, AsyncRequestHandler, AsyncRequestHandlerRelay, AsyncRouter, InputCtx, Requestable, RequestFrame, ResponseFrame, Wave, WaveFrame};
use mesh_portal_versions::version::v0_0_1::quota::Timeouts;

pub struct Shell {
    messenger: Arc<dyn AsyncMessenger<RequestFrame,ResponseFrame>>,
    handlers: ShellHandler,
    logger: RootLogger,
    exchanges: Arc<DashMap<Uuid,oneshot::Sender<ResponseFrame>>>,
    outlet_tx: mpsc::Sender<WaveFrame>
}

impl Shell {
    pub async fn new( point: Point, messenger: Arc<dyn AsyncMessenger<Request,Response>>, mut inlet_rx: mpsc::Receiver<WaveFrame>, outlet_tx: mpsc::Sender<WaveFrame>, logger: RootLogger ) -> Self {
        let logger = logger.point(point.clone());
        let port = point.to_port().with_layer(Layer::Shell );
        let messenger = AsyncMessengerAgent::new( Agent::Anonymous, port.clone(), messenger );
        let exchanges = Arc::new(DashMap::new());
        let handlers = AsyncInternalRequestHandlers::new();
        {
            let handlers =  handlers.clone();
            let port = port.clone();
            let outlet_tx = outlet_tx.clone();
            let exchanges = exchanges.clone();
            let core_messenger = messenger.with_from( port.with_layer(Layer::Core));
            tokio::spawn(async move {
                while let Ok(frame) = inlet_rx.recv().await {
                    // first make sure from() is the expected assigned core port
                    if frame.from().point != port.point {
                        logger.span().error( "particle core attempted to send a message with a from point other than it's own.");
                        continue;
                    }
                    match frame {
                        WaveFrame::Request(frame) => {
                            let stub = frame.as_stub();
                            if frame.to().point == port.point {
                                if frame.to().layer == Layer::Shell {
                                    let request = frame.request;
                                    let logger = logger.opt_span(frame.span);
                                    let ctx = RootRequestCtx::new( request, logger.clone() );
                                    let response: ResponseCore = handlers.handle(ctx).await.into();
                                    let frame = stub.core(response);
                                    let frame: WaveFrame = frame.into();
                                    outlet_tx.send( frame ).await;
                                } else {
                                    // sure, the core can send a message to itself...
                                    outlet_tx.send(frame.into() ).await;
                                }
                            } else {
                                let frame : ResponseFrame = stub.result(core_messenger.send( frame.request.into() ).await);
                                let frame: WaveFrame = frame.into();
                                outlet_tx.send(frame).await;
                            }
                        }
                        WaveFrame::Response(frame) => {
                            if frame.to().point == port.point {
                                if frame.to().layer == Layer::Shell {
                                    match exchanges.remove(frame.response_to() ) {
                                        Some((_,mut tx)) => tx.send(frame).await,
                                        None => { }
                                    }
                                } else {
                                    // just responding to itself?
                                    outlet_tx.send(frame.into())
                                }
                            } else {
                                let response :Response = frame.into();
                                core_messenger.route( response.into() ).await;
                            }
                        }
                    }

                    if let Layer::Shell = frame.to().layer {

                    } else {

                    }
                }
            });
        }

        let cli = Cli::new(messenger.clone().with_from(port.clone().with_layer(Layer::Core)));
        let cli_relay = CliRelay::new( port.clone(), messenger.clone() );
        handlers.add( RouteSelector::any(), AsyncRequestHandlerRelay::new(Box::new(cli_relay)) );
        let handlers = ShellHandler::new(handlers);

        Self {
            port,
            handlers,
            logger,
            exchanges,
            messenger,
            outlet_tx
        }
    }

}


#[derive(AsyncRequestHandler)]
pub struct ShellHandler {
    handlers: AsyncInternalRequestHandlers<AsyncRequestHandlerRelay>,
    outlet_tx: mpsc::Sender<WaveFrame>,
    exchanges: Arc<DashMap<Uuid,oneshot::Sender<Response>>>,
    timeouts: Timeouts
}


impl ShellHandler {
    pub fn new( handlers: AsyncInternalRequestHandlers<AsyncRequestHandlerRelay>, outlet_tx: mpsc::Sender<WaveFrame>, exchanges: Arc<DashMap<Uuid,oneshot::Sender<Response>>> ) -> Self {
        Self {
            handlers,
            outlet_tx,
            exchanges,
            timeouts: Default::default()
        }
    }
}

#[routes(self.handlers)]
impl ShellHandler {
    #[route_async(<*>)]
    pub async fn any(&self, ctx: InputCtx<'_,Request> ) -> Result<ResponseCore,MsgErr> {
        let (tx,rx) = oneshot::channel();
        self.exchanges.insert( ctx.input.id.clone(), tx );
        self.outlet_tx.send( (*ctx.input).into() ).await;
        if let Ok(frame) = tokio::time::timeout( Duration::from_secs(self.timeouts.from(self.timeouts.from(&ctx.input.handling.wait) )), rx).await {
            if let Ok(response) = frame {
                Ok(response.core)
            }
        }
        Ok(ctx.server_error().into())
    }
}