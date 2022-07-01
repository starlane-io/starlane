use crate::star::{StarInjectTransmitter, StarSkel, TopicHandler};
use crate::state::ShellState;
use dashmap::mapref::one::Ref;
use dashmap::DashMap;
use mesh_portal_versions::error::MsgErr;
use mesh_portal_versions::version::v0_0_1::cli::RawCommand;
use mesh_portal_versions::version::v0_0_1::config::config::bind::RouteSelector;
use mesh_portal_versions::version::v0_0_1::id::id::{
    Layer, Point, Port, PortSelector, ToPoint, ToPort, Topic, TraversalLayer, Uuid,
};
use mesh_portal_versions::version::v0_0_1::id::{Traversal, TraversalInjection};
use mesh_portal_versions::version::v0_0_1::log::RootLogger;
use mesh_portal_versions::version::v0_0_1::parse::{command_line, Env};
use mesh_portal_versions::version::v0_0_1::quota::Timeouts;
use mesh_portal_versions::version::v0_0_1::wave::{Agent, PointRequestHandler, PointDirectedHandlerSelector, DirectedHandler, AsyncRequestHandlerRelay, AsyncRouter, Transmitter, InCtx, Ping, WaveXtra, DirectedHandler, Reflectable, ReflectedCore, Pong, RespXtra, RootInCtx, Wave, WaveXtra, ProtoTransmitter, DirectedCore, PingProto, SetStrategy};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;
use cosmic_nom::new_span;
use mesh_portal_versions::version::v0_0_1::command::Command;
use mesh_portal_versions::version::v0_0_1::parse::error::result;
use mesh_portal_versions::version::v0_0_1::util::ToResolved;

#[derive(AsyncRequestHandler)]
pub struct ShellEx {
    skel: StarSkel,
    state: ShellState,
}

impl ShellEx {
    pub fn new(skel: StarSkel, state: ShellState) -> Self {
        Self { skel, state }
    }
}

#[async_trait]
impl TraversalLayer for ShellEx {
    fn port(&self) -> &Port{
        &self.state.port
    }

    async fn traverse_next(&self, traversal: Traversal<Wave>) {
        self.skel.traverse_to_next.send(traversal).await;
    }

    async fn inject(&self, inject: TraversalInjection) {
        self.skel.inject_tx.send(inject).await;
    }

    fn exchange(&self) -> &Arc<DashMap<Uuid, oneshot::Sender<Pong>>> {
        &self.skel.exchange
    }

    async fn deliver_request(&self, request: Ping) {
        let logger = self.skel.logger.point(request.to.point.clone()).span();
        let injector = request.from.clone().with_topic(Topic::None).with_layer(self.layer().clone());
        let transmitter = Arc::new(StarInjectTransmitter::new(
            self.skel.clone(),
          injector.clone()
        ));

        let mut transmitter = ProtoTransmitter::new(transmitter);
        transmitter.from = SetStrategy::Fill(request.from.with_layer(self.layer().clone() ).with_topic(Topic::None));
        let ctx = RootInCtx::new(request, logger, transmitter.clone());
        let response: Result<Pong, MsgErr> = self.handle(ctx).await;
        let wave: Wave = response.into();
        self.inject( TraversalInjection::new(injector,wave)).await;
    }

    async fn request_fabric_bound(&self, traversal: Traversal<Ping>) {
        self.state.fabric_requests.insert(traversal.id.clone());
        self.traverse_next(traversal.wrap()).await;
    }

    async fn response_core_bound(&self, traversal: Traversal<Pong>) {
        if let Some(_) = self.state.fabric_requests.remove(&traversal.response_to) {
            self.traverse_next(traversal.wrap()).await;
        } else {
            traversal.logger.warn("filtered a response to a request of which the Shell has no record");
        }
    }


}

#[routes]
impl ShellEx {
    #[route("Msg<NewCli>")]
    pub async fn new_session(&self, ctx: InCtx<'_, Ping>) -> Result<Port, MsgErr> {
        // only allow a cli session to be created by any layer of THIS particle
        if ctx.from.clone().to_point() != ctx.to.clone().to_point() {
            return Err(MsgErr::forbidden());
        }

        let mut session_port = ctx
            .to
            .clone()
            .with_topic(Topic::uuid())
            .with_layer(Layer::Shell);

        let env = Env::new(ctx.to.clone().to_point());

        let session = CliSession {
            source_selector: ctx.from.clone().into(),
            env,
            port: session_port.clone()
        };

        self.skel
            .state
            .topic
            .insert(session_port.clone(), Box::new(session));

        Ok(session_port)
    }
}

#[routes_async]
impl CliSession {
    #[route("Msg<Exec>")]
    pub async fn exec(&self, ctx: InCtx<'_, RawCommand>) -> Result<ReflectedCore, MsgErr> {
        let exec_topic = Topic::uuid();
        let exec_port = self.port.clone().with_topic(exec_topic.clone());
        let mut exec = CommandExecutor::new(
            exec_port,
            ctx.wave().from.clone(),
            self.env.clone(),
        );

        let result = exec.execute(ctx).await;

        result
    }
}

#[derive(AsyncRequestHandler)]
pub struct CliSession {
    pub source_selector: PortSelector,
    pub env: Env,
    pub port: Port,
}

impl TopicHandler for CliSession {
    fn source_selector(&self) -> &PortSelector {
        &self.source_selector
    }
}


#[derive(AsyncRequestHandler)]
pub struct CommandExecutor {
    port: Port,
    source: Port,
    env: Env,
}

#[routes_async]
impl CommandExecutor {
    pub fn new(port: Port, source: Port, env: Env) -> Self {
        Self {
            port,
            source,
            env,
        }
    }

    pub async fn execute(&self, ctx: InCtx<'_, RawCommand>) -> Result<ReflectedCore, MsgErr> {
        // make sure everything is coming from this command executor topic
        let ctx = ctx.push_from( self.port.clone() );

        let command = result(command_line(new_span(ctx.line.as_str())))?;
        let mut env = self.env.clone();
        for transfer in &ctx.transfers {
            env.set_file(transfer.id.clone(), transfer.content.clone())
        }
        let command: Command = command.to_resolved(&self.env)?;
        let request: DirectedCore = command.into();
        let request = PingProto::from_core(request);

        Pong::core_result(ctx.req(request).await)
    }
}
