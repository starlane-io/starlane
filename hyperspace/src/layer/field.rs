use crate::base::Platform;
use crate::star::{HyperStarSkel, TraverseToNextRouter};
use async_trait::async_trait;
use starlane_macros::push_loc;
use starlane_space::artifact::asynch::ArtErr;
use starlane_space::artifact::ArtRef;
use starlane_space::config::bind::{BindConfig, PipelineStepVar, PipelineStopVar};
use starlane_space::err::{CoreReflector, LegacyStatusErr, SpaceErr};
use starlane_space::loc::{Layer, Surface, ToSurface};
use starlane_space::log::{Logger, Trackable};
use starlane_space::parse::model::{PipelineSegmentVar, PipelineVar};
use starlane_space::parse::{Env, RegexCapturesResolver};
use starlane_space::particle::traversal::{Traversal, TraversalLayer};
use starlane_space::point::Point;
use starlane_space::selector::PayloadBlock;
use starlane_space::substance::Substance;
use starlane_space::util::{log, ToResolved};
use starlane_space::wave::core::{Method, ReflectedCore};
use starlane_space::wave::exchange::asynch::ProtoTransmitter;
use starlane_space::wave::exchange::asynch::{Exchanger, TraversalTransmitter};
use starlane_space::wave::{
    BounceBacks, DirectedKind, DirectedProto, DirectedWave, EchoCore, PongCore, Reflection, Wave,
    WaveVariantDef,
};
use std::str::FromStr;
use std::sync::Arc;
use tracing_core::Subscriber;
use url::Url;

pub struct Field {
    pub port: Surface,
    pub skel: HyperStarSkel,
    pub logger: Logger,
    pub shell_transmitter: TraversalTransmitter,
}

impl Field {
    pub fn new(point: Point, skel: HyperStarSkel) -> Self {
        let port = point.to_surface().with_layer(Layer::Field);
        let logger = push_loc!((skel.logger, &port));
        let shell_router = Arc::new(TraverseToNextRouter::new(skel.traverse_to_next_tx.clone()));
        let shell_transmitter = TraversalTransmitter::new(shell_router, skel.exchanger.clone());

        Self {
            port,
            skel,
            logger,
            shell_transmitter,
        }
    }

    async fn bind(&self, directed: &Traversal<DirectedWave>) -> Result<ArtRef<BindConfig>, ArtErr> {
        let record = self
            .skel
            .registry
            .record(&self.port.point)
            .await
            .map_err(anyhow::Error::from)?;

        let properties = self
            .skel
            .registry
            .get_properties(&directed.to.point)
            .await
            .map_err(anyhow::Error::from)?;

        let bind_property = properties.get("bind");
        let bind = match bind_property {
            None => {
                let driver = self
                    .skel
                    .drivers
                    .get(&record.details.stub.kind)
                    .await
                    .map_err(anyhow::Error::from)?;
                driver
                    .bind(&directed.to.point)
                    .await
                    .map_err(anyhow::Error::from)?
            }
            Some(bind) => {
                let bind = Point::from_str(bind.value.as_str()).map_err(anyhow::Error::from)?;
                log(self.skel.machine_api.artifacts.get_bind(&bind).await)
                    .map_err(anyhow::Error::from)?
            }
        };
        Ok(bind)
    }

    pub fn pipex(&self, traversal: Traversal<DirectedWave>, pipeline: PipelineVar, env: Env) {
        PipeEx::new(
            self.skel.clone(),
            self.port.clone(),
            traversal,
            pipeline,
            env,
            self.shell_transmitter.clone(),
            self.skel.gravity_transmitter.clone(),
            self.logger.clone(),
        );
    }
}

#[async_trait]
impl TraversalLayer for Field {
    fn surface(&self) -> Surface {
        self.port.clone()
    }

    async fn traverse_next(&self, traversal: Traversal<Wave>) {
        self.logger
            .result(self.skel.traverse_to_next_tx.send(traversal).await);
    }

    async fn inject(&self, wave: Wave) {
        panic!("cannot inject here!");
        //        self.shell_transmitter.route(wave).await;
    }

    fn exchanger(&self) -> &Exchanger {
        &self.skel.exchanger
    }

    async fn directed_core_bound(&self, directed: Traversal<DirectedWave>) -> Result<(), SpaceErr> {
        let bind = self.bind(&directed).await?;

        match bind.select(&directed.payload) {
            Ok(route) => {
                let regex = route.selector.path.clone();
                let env = {
                    let path_regex_capture_resolver =
                        RegexCapturesResolver::new(regex, directed.core().uri.path().to_string())?;
                    let mut env = Env::new(self.port.point.clone());
                    env.add_var_resolver(Arc::new(path_regex_capture_resolver));
                    env.set_var("doc.bundle", bind.bundle().clone().into());
                    env.set_var("doc", bind.point().clone().into());
                    env
                };
                self.pipex(directed, route.block.clone(), env);
                Ok(())
            }
            Err(err) => {
                if let Method::Cmd(cmd) = &directed.core().method {
                    let mut pipeline = PipelineVar::new();
                    pipeline.segments.push(PipelineSegmentVar {
                        step: PipelineStepVar::direct(),
                        stop: PipelineStopVar::Core,
                    });
                    pipeline.segments.push(PipelineSegmentVar {
                        step: PipelineStepVar::rtn(),
                        stop: PipelineStopVar::Reflect,
                    });
                    let env = {
                        let mut env = Env::new(self.port.point.clone());
                        env.set_var("doc.bundle", bind.bundle().clone().into());
                        env.set_var("doc", bind.point().clone().into());
                        env
                    };
                    self.pipex(directed, pipeline, env);
                    Ok(())
                } else {
                    if err.status() == 404u16 {
                        Err(SpaceErr::new(
                            404,
                            format!(
                                "no route matches: {} on surface {} and bind {} from {}",
                                directed.core().to_selection_str(),
                                directed.to.to_string(),
                                bind.point.to_string(),
                                directed.from().to_string()
                            ),
                        ))
                    } else {
                        Err(err)
                    }
                }
            }
        }
    }
}

pub struct PipeEx {
    pub skel: HyperStarSkel,
    pub surface: Surface,
    pub logger: Logger,
    pub env: Env,
    pub reflection: Result<Reflection, SpaceErr>,
    pub pipeline: PipelineVar,
    pub shell_transmitter: TraversalTransmitter,
    pub gravity_transmitter: ProtoTransmitter,
    pub traversal: Traversal<DirectedWave>,

    pub kind: DirectedKind,
    pub method: Method,
    pub uri: Url,
    pub body: Substance,
    pub status: u16,
}

impl PipeEx {
    pub fn new(
        skel: HyperStarSkel,
        port: Surface,
        traversal: Traversal<DirectedWave>,
        pipeline: PipelineVar,
        env: Env,
        shell_transmitter: TraversalTransmitter,
        gravity_transmitter: ProtoTransmitter,
        logger: Logger,
    ) {
        tokio::spawn(async move {
            let pipex = Self {
                skel,
                kind: traversal.directed_kind(),
                method: traversal.core().method.clone(),
                uri: traversal.core().uri.clone(),
                body: traversal.core().body.clone(),
                reflection: traversal.reflection(),
                surface: port,
                traversal,
                env,
                pipeline,
                shell_transmitter,
                gravity_transmitter,
                logger,
                status: 200u16,
            };
            pipex.start().await;
        });
    }

    pub async fn start(mut self) {
        match self.execute().await {
            Ok(_) => {}
            Err(err) => {
                self.logger.error(format!("{}", err.to_string()));
                match &self.reflection {
                    Ok(reflection) => {
                        let wave = reflection
                            .clone()
                            .make(err.as_reflected_core(), self.surface.clone());
                        let wave = wave.to_wave();
                        self.gravity_transmitter.route(wave).await;
                    }
                    Err(_) => {}
                }
            }
        }
    }

    fn proto(&self) -> DirectedProto {
        let mut proto = DirectedProto::kind(&self.kind);
        proto.id = self.traversal.id().clone();
        proto.via(&self.surface);
        proto.method(self.method.clone());
        proto.body(self.body.clone());
        proto.uri(self.uri.clone());
        proto.handling(self.traversal.handling().clone());
        proto.agent(self.traversal.agent().clone());
        proto.bounce_backs(self.traversal.bounce_backs().clone());
        proto.scope(self.traversal.scope().clone());
        proto.from(self.traversal.from().clone());
        proto.history(self.traversal.history());
        proto.track = self.traversal.track();

        proto
    }

    pub async fn execute(&mut self) -> Result<(), SpaceErr> {
        while let Some(segment) = self.pipeline.consume() {
            self.execute_step(&segment.step)?;
            self.execute_stop(&segment.stop).await?;
        }
        Ok(())
    }

    fn execute_step(&mut self, step: &PipelineStepVar) -> Result<(), SpaceErr> {
        for block in &step.blocks {
            match block.clone().to_resolved(&self.env)? {
                PayloadBlock::DirectPattern(pattern) => {
                    pattern.is_match(&self.body)?;
                }
                PayloadBlock::ReflectPattern(pattern) => {
                    pattern.is_match(&self.body)?;
                }
            }
        }
        Ok(())
    }

    async fn execute_stop(&mut self, stop: &PipelineStopVar) -> Result<(), SpaceErr> {
        match stop {
            PipelineStopVar::Core => {
                let mut proto = self.proto();
                proto.to(self.surface.with_layer(Layer::Core));
                let directed = proto.build()?;
                let traversal = self.traversal.clone().with(directed);
                self.traverse_to_next(traversal, self.shell_transmitter.clone())
                    .await
            }
            PipelineStopVar::Reflect => {
                let reflection = self.reflection.clone()?;
                let mut core = ReflectedCore::status(self.status);
                core.body = self.body.clone();

                let reflected = reflection.make(core, self.traversal.to.clone());

                self.gravity_transmitter.route(reflected.to_wave()).await;
                Ok(())
            }
            PipelineStopVar::Call(_) => {
                unimplemented!()
            }
            PipelineStopVar::Point(point) => {
                let point: Point = point.clone().to_resolved(&self.env)?;
                let mut proto = self.proto();
                proto.to(point.to_surface().with_layer(Layer::Core));

                self.direct(proto, self.gravity_transmitter.clone()).await
            }
            PipelineStopVar::Err { .. } => {
                unimplemented!()
            }
        }
    }

    async fn direct(
        &mut self,
        mut proto: DirectedProto,
        transmitter: ProtoTransmitter,
    ) -> Result<(), SpaceErr> {
        match proto.kind.as_ref().unwrap() {
            DirectedKind::Ping => {
                let pong: WaveVariantDef<PongCore> = transmitter.direct(proto).await?;
                self.status = pong.core.status.as_u16();
                if pong.core.status.is_success() {
                    self.body = pong.core.body.clone();
                    Ok(())
                } else {
                    Err(pong.core.to_err())
                }
            }
            DirectedKind::Ripple => {
                // this should be a single echo since in traversal it is only going to a single target
                if proto.bounce_backs.is_some() {
                    proto.bounce_backs(BounceBacks::Count(1));
                }
                let mut echoes: Vec<WaveVariantDef<EchoCore>> = transmitter.direct(proto).await?;
                if !echoes.is_empty() {
                    let echo = echoes.remove(0);
                    self.status = echo.core.status.as_u16();
                    if echo.core.status.is_success() {
                        self.body = echo.core.body.clone();
                        Ok(())
                    } else {
                        Err(echo.core.to_err())
                    }
                } else {
                    Ok(())
                }
            }
            DirectedKind::Signal => {
                transmitter.direct(proto).await?;
                Ok(())
            }
        }
    }

    async fn traverse_to_next(
        &mut self,
        mut proto: Traversal<DirectedWave>,
        transmitter: TraversalTransmitter,
    ) -> Result<(), SpaceErr> {
        match proto.directed_kind() {
            DirectedKind::Ping => {
                let pong: WaveVariantDef<PongCore> = transmitter.direct(proto).await?;
                self.status = pong.core.status.as_u16();
                if pong.core.status.is_success() {
                    self.body = pong.core.body.clone();
                    Ok(())
                } else {
                    Err(pong.core.to_err())
                }
            }
            DirectedKind::Ripple => {
                // this should be a single echo since in traversal it is only going to a single target
                proto.set_bounce_backs(BounceBacks::Count(1));
                let mut echoes: Vec<WaveVariantDef<EchoCore>> = transmitter.direct(proto).await?;
                if !echoes.is_empty() {
                    let echo = echoes.remove(0);
                    self.status = echo.core.status.as_u16();
                    if echo.core.status.is_success() {
                        self.body = echo.core.body.clone();
                        Ok(())
                    } else {
                        Err(echo.core.to_err())
                    }
                } else {
                    Ok(())
                }
            }
            DirectedKind::Signal => {
                transmitter.direct(proto).await?;
                Ok(())
            }
        }
    }
}
