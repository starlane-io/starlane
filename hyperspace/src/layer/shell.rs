use crate::base::Platform;
use crate::star::{HyperStarSkel, LayerInjectionRouter, TopicHandler};
use async_trait::async_trait;
use dashmap::{DashMap, DashSet};
use enum_ordinalize::Ordinalize;
use lazy_static::lazy_static;
use starlane_macros::{handler, push_loc, route, DirectedHandler};
use starlane_space::command::common::StateSrc;
use starlane_space::command::{Command, RawCommand};
use starlane_space::err::SpaceErr;
use starlane_space::loc::{Layer, Surface, SurfaceSelector, ToPoint, ToSurface, Topic};
use starlane_space::log::Logger;
use starlane_space::parse::util::new_span;
use starlane_space::parse::util::result;
use starlane_space::parse::{command_line, Env};
use starlane_space::particle::traversal::{Traversal, TraversalInjection, TraversalLayer};
use starlane_space::point::Point;
use starlane_space::substance::Substance;
use starlane_space::util::ToResolved;
use starlane_space::wave::core::{CoreBounce, DirectedCore, ReflectedCore};
use starlane_space::wave::exchange::asynch::{
    DirectedHandler, Exchanger, InCtx, ProtoTransmitterBuilder, RootInCtx,
};
use starlane_space::wave::exchange::SetStrategy;
use starlane_space::wave::{DirectedProto, DirectedWave, PongCore, Wave, WaveId, WaveVariantDef};
use std::sync::atomic::AtomicU16;
use std::sync::Arc;

#[derive(DirectedHandler)]
pub struct Shell {
    skel: HyperStarSkel,
    state: ShellState,
    logger: Logger,
}

impl Shell {
    pub fn new(skel: HyperStarSkel, state: ShellState) -> Self {
        let logger = push_loc!((skel.logger, &state.point));
        Self {
            skel,
            state,
            logger,
        }
    }
}

#[async_trait]
impl TraversalLayer for Shell {
    fn surface(&self) -> Surface {
        self.state
            .point
            .clone()
            .to_surface()
            .with_layer(Layer::Shell)
    }

    async fn traverse_next(&self, traversal: Traversal<Wave>) {
        self.skel.traverse_to_next_tx.send(traversal.clone()).await;
    }

    async fn inject(&self, wave: Wave) {
        let inject = TraversalInjection::new(self.surface().clone(), wave);
        self.skel.inject_tx.send(inject).await;
    }

    fn exchanger(&self) -> &Exchanger {
        &self.skel.exchanger
    }

    async fn deliver_directed(&self, directed: Traversal<DirectedWave>) -> Result<(), SpaceErr> {
        if directed.from().point == self.surface().point
            && directed.from().layer.ordinal() >= self.surface().layer.ordinal()
        {
            self.state
                .fabric_requests
                .insert(directed.id().clone(), AtomicU16::new(1));
        }

        let logger = push_loc!((self.skel.logger, &directed.to));
        let injector = directed
            .from()
            .clone()
            .with_topic(directed.to.topic.clone())
            .with_layer(self.surface().layer.clone());
        let router = Arc::new(LayerInjectionRouter::new(
            self.skel.clone(),
            injector.clone(),
        ));

        let mut transmitter =
            ProtoTransmitterBuilder::new(router.clone(), self.exchanger().clone());
        transmitter.from = SetStrategy::Fill(
            directed
                .from()
                .with_layer(self.surface().layer.clone())
                .with_topic(directed.to.topic.clone()),
        );
        let transmitter = transmitter.build();
        let reflection = directed.reflection();
        let ctx = RootInCtx::new(
            directed.payload.clone(),
            self.surface().clone(),
            logger,
            transmitter.clone(),
        );

        let bounce: CoreBounce = if directed.to.topic == Topic::None {
            self.handle(ctx).await
        } else {
            let handler = self
                .skel
                .state
                .find_topic(&directed.to, directed.from())
                .ok_or("expecting topic")??;
            handler.handle(ctx).await
        };

        match bounce {
            CoreBounce::Absorbed => {}
            CoreBounce::Reflected(core) => {
                let reflected = reflection.unwrap().make(core, self.surface().clone());
                self.inject(reflected.to_wave()).await;
            }
        }
        Ok(())
    }

    /*
    async fn directed_fabric_bound(
        &self,
        mut traversal: Traversal<DirectedWave>,
    ) -> Result<(), SpaceErr> {
        match traversal.directed_kind() {
            DirectedKind::Ping => {
                //self.logger.info(format!("Shell tracking id: {} to: {}",traversal.id().to_short_string(), traversal.to.to_string()) );
                self.state
                    .fabric_requests
                    .insert(traversal.id().clone(), AtomicU16::new(1));
            }
            DirectedKind::Ripple => {
                match traversal.bounce_backs() {
                    BounceBacks::None => {}
                    BounceBacks::Single => {
                        self.state
                            .fabric_requests
                            .insert(traversal.id().clone(), AtomicU16::new(1));
                    }
                    BounceBacks::Count(c) => {
                        self.state
                            .fabric_requests
                            .insert(traversal.id().clone(), AtomicU16::new(c as u16));
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
    ) -> Result<(), SpaceErr> {
        if let Some(count) = self.state.fabric_requests.get(traversal.reflection_of()) {
            let value = count.value().fetch_sub(1, Ordering::Relaxed);
            if value >= 0 {
                self.traverse_next(traversal.clone().to_wave()).await;
            } else {
                self.logger.warn(format!(
                    "{} blocked a reflected from {} to a directed id {} of which the Shell has already received a reflected wave",
                    self.surface().to_string(),
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
                self.surface().to_string(),
                traversal.from().to_string(),
                traversal.reflection_of().to_short_string()
            ));
        }
        Ok(())
    }

     */
}

#[handler]
impl Shell {
    #[route("Ext<NewCliSession>")]
    pub async fn new_session(&self, ctx: InCtx<'_, ()>) -> Result<Surface, SpaceErr> {
        // only allow a cli session to be created by any layer of THIS particle
        if ctx.from().clone().to_point() != ctx.to().clone().to_point() {
            return Err(SpaceErr::forbidden(
                "cli sessions can only be created from within the same Point",
            ));
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

#[handler]
impl CliSession {
    #[route("Ext<Exec>")]
    pub async fn exec(&self, ctx: InCtx<'_, RawCommand>) -> Result<ReflectedCore, SpaceErr> {
        let exec_topic = Topic::uuid();
        let exec_port = self.port.clone().with_topic(exec_topic.clone());
        let mut exec = CommandExecutor::new(exec_port, ctx.from().clone(), self.env.clone());

        Ok(exec.execute(ctx).await?)
    }
}

#[derive(DirectedHandler)]
pub struct CliSession {
    pub source_selector: SurfaceSelector,
    pub env: Env,
    pub port: Surface,
}

impl TopicHandler for CliSession {
    fn source_selector(&self) -> &SurfaceSelector {
        &self.source_selector
    }
}

#[derive(DirectedHandler)]
pub struct CommandExecutor {
    port: Surface,
    source: Surface,
    env: Env,
}

#[handler]
impl CommandExecutor {
    pub fn new(port: Surface, source: Surface, env: Env) -> Self {
        Self { port, source, env }
    }

    pub async fn execute(&self, ctx: InCtx<'_, RawCommand>) -> Result<ReflectedCore, SpaceErr> {
        // make sure everything is coming from this command executor topic
        let ctx = ctx.push_from(self.port.clone());

        let command = result(command_line(new_span(ctx.line.as_str())))?;

        let mut env = self.env.clone();
        for transfer in &ctx.transfers {
            env.set_file(transfer.id.clone(), transfer.content.clone())
        }
        let mut command: Command = command.to_resolved(&self.env)?;

        if let Command::Create(create) = &mut command {
            if ctx.transfers.len() == 1 {
                let transfer = ctx.transfers.get(0).unwrap().clone();
                create.state = StateSrc::Subst(Box::new(Substance::Bin(transfer.content)));
            } else if ctx.transfers.len() > 1 {
                return Err("create cannot handle more than one state transfer".into());
            }
        }

        let request: DirectedCore = command.into();
        let mut directed = DirectedProto::from_core(request);
        directed.to(Point::global_executor());
        let pong: WaveVariantDef<PongCore> = ctx.transmitter.direct(directed).await?;

        Ok(pong.variant.core)
    }
}

#[derive(Clone)]
pub struct ShellState {
    pub point: Point,
    pub core_requests: Arc<DashSet<WaveId>>,
    pub fabric_requests: Arc<DashMap<WaveId, AtomicU16>>,
}

impl ShellState {
    pub fn new(point: Point) -> Self {
        Self {
            point,
            core_requests: Arc::new(DashSet::new()),
            fabric_requests: Arc::new(DashMap::new()),
        }
    }
}
