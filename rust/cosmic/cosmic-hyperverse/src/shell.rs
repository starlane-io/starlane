use crate::star::{LayerInjectionRouter, HyperStarSkel, TopicHandler};
use crate::state::ShellState;
use crate::{PlatErr, Platform};
use cosmic_universe::cli::RawCommand;
use cosmic_universe::command::Command;
use cosmic_universe::config::config::bind::RouteSelector;
use cosmic_universe::error::UniErr;
use cosmic_universe::id::id::{
    Layer, Point, Port, PortSelector, ToPoint, ToPort, Topic, TraversalLayer, Uuid,
};
use cosmic_universe::id::{Traversal, TraversalDirection, TraversalInjection};
use cosmic_universe::log::{PointLogger, RootLogger, Trackable};
use cosmic_universe::parse::error::result;
use cosmic_universe::parse::{command_line, route_attribute, Env};
use cosmic_universe::quota::Timeouts;
use cosmic_universe::util::{log, ToResolved};
use cosmic_universe::wave::{Agent, Bounce, BounceBacks, CoreBounce, DirectedCore, DirectedHandler, DirectedHandlerSelector, DirectedKind, DirectedProto, DirectedWave, Exchanger, InCtx, Ping, Pong, ProtoTransmitter, ProtoTransmitterBuilder, RecipientSelector, Reflectable, ReflectedCore, ReflectedWave, RootInCtx, Router, SetStrategy, UltraWave, Wave, WaveKind};
use cosmic_nom::new_span;
use dashmap::mapref::one::Ref;
use dashmap::DashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;

#[derive(DirectedHandler)]
pub struct Shell<P>
where
    P: Platform + 'static,
{
    skel: HyperStarSkel<P>,
    state: ShellState,
    logger: PointLogger
}

impl<P> Shell<P>
where
    P: Platform + 'static,
{
    pub fn new(skel: HyperStarSkel<P>, state: ShellState) -> Self {
        let logger = skel.logger.point(state.point.clone());
        Self { skel, state, logger }
    }
}

#[async_trait]
impl<P> TraversalLayer for Shell<P>
where
    P: Platform + 'static,
{
    fn port(&self) -> Port {
        self.state.point.clone().to_port().with_layer(Layer::Shell)
    }

    async fn traverse_next(&self, traversal: Traversal<UltraWave>) {
        self.skel.traverse_to_next_tx.send(traversal.clone()).await;
    }

    async fn inject(&self, wave: UltraWave) {
        let inject = TraversalInjection::new(self.port().clone(), wave);
        self.skel.inject_tx.send(inject).await;
    }

    fn exchanger(&self) -> &Exchanger {
        &self.skel.exchanger
    }

    async fn deliver_directed(&self, directed: Traversal<DirectedWave>) -> Result<(), UniErr> {
        if directed.from().point == self.port().point
            && directed.from().layer.ordinal() >= self.port().layer.ordinal()
        {
            self.state.fabric_requests.insert(directed.id().clone(), AtomicU16::new(1));
        }

        let logger = self.skel.logger.point(directed.to.point.clone()).span();
        let injector = directed
            .from()
            .clone()
            .with_topic(directed.to.topic.clone())
            .with_layer(self.port().layer.clone());
        let router = Arc::new(LayerInjectionRouter::new(
            self.skel.clone(),
            injector.clone(),
        ));

        let mut transmitter =
            ProtoTransmitterBuilder::new(router.clone(), self.exchanger().clone());
        transmitter.from = SetStrategy::Fill(
            directed
                .from()
                .with_layer(self.port().layer.clone())
                .with_topic(directed.to.topic.clone()),
        );
        let transmitter = transmitter.build();
        let reflection = directed.reflection();
        let ctx = RootInCtx::new(
            directed.payload.clone(),
            self.port().clone(),
            logger,
            transmitter.clone(),
        );

        let bounce: CoreBounce = if directed.to.topic == Topic::None {
            self.handle(ctx).await
        } else {
println!("Handling Topic");
            let handler = self.skel.state.find_topic(&directed.to, directed.from() ).ok_or("expecting topic")??;
            handler.handle(ctx).await
        };


        match bounce {
            CoreBounce::Absorbed => {}
            CoreBounce::Reflected(core) => {
                let reflected = reflection.unwrap().make(core, self.port().clone());
                self.inject(reflected.to_ultra()).await;
            }
        }
        Ok(())
    }

    async fn directed_fabric_bound(
        &self,
        mut traversal: Traversal<DirectedWave>,
    ) -> Result<(), UniErr> {

        match traversal.directed_kind() {
            DirectedKind::Ping => {
//self.logger.info(format!("Shell tracking id: {} to: {}",traversal.id().to_short_string(), traversal.to.to_string()) );
                self.state.fabric_requests.insert(traversal.id().clone(), AtomicU16::new(1));
            }
            DirectedKind::Ripple => {
                match traversal.bounce_backs() {
                    BounceBacks::None => {}
                    BounceBacks::Single => {
                        self.state.fabric_requests.insert(traversal.id().clone(), AtomicU16::new(1));
                    }
                    BounceBacks::Count(c) => {
                        self.state.fabric_requests.insert(traversal.id().clone(), AtomicU16::new(c as u16));
                    }
                    BounceBacks::Timer(_) => {
                        // not sure what to do in this case...
                    }
                }
            }
            DirectedKind::Signal => {}
        }

        self.traverse_next(traversal.wrap()).await;
        Ok(())
    }

    async fn reflected_core_bound(
        &self,
        traversal: Traversal<ReflectedWave>,
    ) -> Result<(), UniErr> {
        // println!("Shell reflected_core_bound: {}", traversal.kind().to_string() );

        if let Some(count) = self
            .state
            .fabric_requests
            .get(traversal.reflection_of())
        {
            let value = count.value().fetch_sub(1, Ordering::Relaxed);
            if value >= 0 {
                self.traverse_next(traversal.clone().to_ultra()).await;
            } else {
                self.logger.warn(format!(
                    "{} blocked a reflected from {} to a directed id {} of which the Shell has already received a reflected wave",
                    self.port().to_string(),
                    traversal.from().to_string(),
                    traversal.reflection_of().to_short_string()
                ));
            }

            if value <= 0 {
                let id = traversal.reflection_of().clone();
                let fabric_requests = self.state.fabric_requests.clone();
                tokio::spawn(async move {
                    fabric_requests.remove(&id);
                });
            }
        } else {
            self.logger.warn(format!(
                "{} blocked a reflected from {} to a directed id {} of which the Shell has no record",
                self.port().to_string(),
                traversal.from().to_string(),
                traversal.reflection_of().to_short_string()
            ));
        }
        Ok(())
    }
}

#[routes]
impl<P> Shell<P>
where
    P: Platform + 'static,
{
    #[route("Msg<NewCliSession>")]
    pub async fn new_session(&self, ctx: InCtx<'_, ()>) -> Result<Port, UniErr> {
        // only allow a cli session to be created by any layer of THIS particle
        if ctx.from().clone().to_point() != ctx.to().clone().to_point() {
            return Err(UniErr::forbidden());
        }

        let mut session_port = ctx
            .to()
            .clone()
            .with_topic(Topic::uuid())
            .with_layer(Layer::Shell);

        let env = Env::new(ctx.to().clone().to_point());

        let session = CliSession {
            source_selector: ctx.from().clone().into(),
            env,
            port: session_port.clone(),
        };

        self.skel
            .state
            .topic_handler(session_port.clone(), Arc::new(session));

        Ok(session_port)
    }
}

#[routes]
impl CliSession {
    #[route("Msg<Exec>")]
    pub async fn exec(&self, ctx: InCtx<'_, RawCommand>) -> Result<ReflectedCore, UniErr> {
println!("---> Reached Msg<Exec> !!!!");
        let exec_topic = Topic::uuid();
        let exec_port = self.port.clone().with_topic(exec_topic.clone());
        let mut exec = CommandExecutor::new(exec_port, ctx.from().clone(), self.env.clone());

        Ok(exec.execute(ctx).await?)
    }
}

#[derive(DirectedHandler)]
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

#[derive(DirectedHandler)]
pub struct CommandExecutor {
    port: Port,
    source: Port,
    env: Env,
}

#[routes]
impl CommandExecutor {
    pub fn new(port: Port, source: Port, env: Env) -> Self {
        Self { port, source, env }
    }

    pub async fn execute(&self, ctx: InCtx<'_, RawCommand>) -> Result<ReflectedCore, UniErr> {
println!("CommadnExecutor...");
        // make sure everything is coming from this command executor topic
        let ctx = ctx.push_from(self.port.clone());

println!("Pre parse line... '{}'",ctx.line);
        let command = log(result(command_line(new_span(ctx.line.as_str()))))?;
println!("post parse line...");
        let mut env = self.env.clone();
        for transfer in &ctx.transfers {
            env.set_file(transfer.id.clone(), transfer.content.clone())
        }
println!("Staring to work...");
        let command: Command = command.to_resolved(&self.env)?;
println!("resolved?...");

        let request: DirectedCore = command.into();
        let mut directed = DirectedProto::from_core(request);
        directed.to(Point::global_executor());
println!("GOT HERE");
        let pong : Wave<Pong> = ctx.transmitter.direct(directed).await?;
        Ok(pong.variant.core)
    }
}
