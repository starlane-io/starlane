use crate::star::{HyperStarSkel, LayerInjectionRouter};
use crate::{PlatErr, Platform};
use cosmic_universe::config::bind::{BindConfig, PipelineStepVar, PipelineStopVar, WaveDirection};
use cosmic_universe::error::{StatusErr, UniErr};
use cosmic_universe::id::Traversal;
use cosmic_universe::wave::{BounceBacks, CmdMethod, DirectedKind, DirectedProto, DirectedWave, Echo, Exchanger, Method, Pong, ProtoTransmitter, ProtoTransmitterBuilder, ReflectedAggregate, ReflectedCore, Reflection, UltraWave, Wave, WaveKind};
use cosmic_universe::artifact::ArtRef;
use std::str::FromStr;
use std::sync::Arc;
use http::Uri;
use cosmic_universe::id::{Layer, Point, Port, ToPoint, ToPort, TraversalLayer};
use cosmic_universe::log::{PointLogger, Trackable};
use cosmic_universe::parse::model::{MethodScope, PipelineSegmentVar, PipelineVar};
use cosmic_universe::parse::{Env, RegexCapturesResolver};
use cosmic_universe::selector::{PayloadBlock, PayloadBlockVar};
use cosmic_universe::substance::Substance;
use cosmic_universe::util::ToResolved;

pub struct Field<P>
where
    P: Platform,
{
    pub port: Port,
    pub skel: HyperStarSkel<P>,
    pub logger: PointLogger,
    pub shell_transmitter: ProtoTransmitter,
}

impl<P> Field<P>
where
    P: Platform,
{
    pub fn new(point: Point, skel: HyperStarSkel<P>) -> Self {
        let port = point.to_port().with_layer(Layer::Field);
        let logger = skel.logger.point(port.point.clone());
        let shell_router = Arc::new(LayerInjectionRouter::new(skel.clone(), port.clone().with_layer(Layer::Shell)));
        let shell_transmitter = ProtoTransmitterBuilder::new(shell_router, skel.exchanger.clone());
        let shell_transmitter = shell_transmitter.build();

        Self {
            port,
            skel,
            logger,
            shell_transmitter,
        }
    }

    async fn bind(&self, directed: &Traversal<DirectedWave>) -> Result<ArtRef<BindConfig>, UniErr> {
        let record = self.skel.registry.record(&self.port.point).await.map_err(|e|e.to_cosmic_err())?;
        let properties = self
            .skel
            .registry
            .get_properties(&directed.to.point)
            .await
            .map_err(|e| e.to_cosmic_err())?;

        let bind_property = properties.get("bind");
        let bind = match bind_property {
            None => {
                let driver = self.skel.drivers.get(&record.details.stub.kind).await?;
                driver
                    .bind(&directed.to.point)
                    .await
                    .map_err(|e| e.to_cosmic_err())?
            }
            Some(bind) => {
                let bind = Point::from_str(bind.value.as_str())?;
                self.skel.machine.artifacts.bind(&bind).await?
            }
        };
        Ok(bind)
    }

    pub fn pipex(&self, traversal: Traversal<DirectedWave>, pipeline: PipelineVar, env: Env)  {
        PipeEx::new(self.port.clone(), traversal, pipeline, env, self.shell_transmitter.clone(),self.skel.gravity_transmitter.clone(),self.logger.clone() );
    }
}

#[async_trait]
impl<P> TraversalLayer for Field<P>
where
    P: Platform,
{
    fn port(&self) -> Port {
        self.port.clone()
    }

    async fn traverse_next(&self, traversal: Traversal<UltraWave>) {
        self.logger
            .eat(self.skel.traverse_to_next_tx.send(traversal).await);
    }

    async fn inject(&self, wave: UltraWave) {
        self.shell_transmitter.route(wave).await;
    }

    fn exchanger(&self) -> &Exchanger {
        &self.skel.exchanger
    }

    async fn directed_core_bound(&self, directed: Traversal<DirectedWave>) -> Result<(), UniErr> {
        let bind = self.bind(&directed).await?;
        match bind.select(&directed.payload) {
            Ok(route) => {
                let regex = route.selector.path.clone();
                let env = {
                    let path_regex_capture_resolver =
                        RegexCapturesResolver::new(regex, directed.core().uri.path().to_string())?;
                    let mut env = Env::new(self.port.point.clone());
                    env.add_var_resolver(Arc::new(path_regex_capture_resolver));
                    env.set_var("self.bundle", bind.bundle().clone().into());
                    env.set_var("self.bind", bind.point().clone().into());
                    env
                };
                self.pipex(directed, route.block.clone(), env);
                Ok(())
            },
            Err(err) => {
                if let Method::Cmd(cmd) = &directed.core().method {
                    let mut pipeline = PipelineVar::new();
                    pipeline.segments.push( PipelineSegmentVar{ step: PipelineStepVar::direct(), stop: PipelineStopVar::Core } );
                    pipeline.segments.push( PipelineSegmentVar{ step: PipelineStepVar::rtn(), stop: PipelineStopVar::Reflect } );
                    let env = {
                        let mut env = Env::new(self.port.point.clone());
                        env.set_var("self.bundle", bind.bundle().clone().into());
                        env.set_var("self.bind", bind.point().clone().into());
                        env
                    };
                    self.pipex(directed, pipeline, env);
                    Ok(())
                } else {
                    Err(err)
                }
            }
        }

    }
}

pub struct PipeEx {
  pub port: Port,
  pub logger: PointLogger,
  pub env: Env,
  pub reflection: Result<Reflection, UniErr>,
  pub pipeline: PipelineVar,
  pub shell_transmitter: ProtoTransmitter,
  pub gravity_transmitter: ProtoTransmitter,
  pub traversal: Traversal<DirectedWave>,

  pub kind: DirectedKind,
  pub method: Method,
  pub uri: Uri,
  pub body: Substance,
  pub status: u16
}

impl PipeEx {
    pub fn new(port: Port, traversal: Traversal<DirectedWave>, pipeline: PipelineVar, env: Env, shell_transmitter: ProtoTransmitter, gravity_transmitter: ProtoTransmitter,logger: PointLogger ) {
        tokio::spawn( async move {

            let pipex = Self {
                kind: traversal.directed_kind(),
                method: traversal.core().method.clone(),
                uri: traversal.core().uri.clone(),
                body: traversal.core().body.clone(),
                reflection: traversal.reflection(),
                port,
                traversal,
                env,
                pipeline,
                shell_transmitter,
                gravity_transmitter,
                logger,
                status: 200u16
            };
            pipex.start().await;
        });
    }

    pub async fn start( mut self ) {
        match self.execute().await {
            Ok(_) => {}
            Err(err) => {
                self.logger.error(format!("{}",err.to_string()));
                match &self.reflection {
                    Ok(reflection) => {

                        let wave = reflection.clone().make(err.as_reflected_core(), self.port.clone() );
                        let wave = wave.to_ultra();
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
        proto.via(Some(self.port.clone()));
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

    pub async fn execute( &mut self ) -> Result<(), UniErr> {
        while let Some( segment ) = self.pipeline.consume() {
            self.execute_step(&segment.step)?;
            self.execute_stop(&segment.stop).await?;
        }
        Ok(())
    }


    fn execute_step(&mut self, step: &PipelineStepVar) -> Result<(), UniErr> {
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

    async fn execute_stop( &mut self, stop: &PipelineStopVar) -> Result<(), UniErr> {
        match stop {
            PipelineStopVar::Core => {
                let mut proto = self.proto();
                proto.to(self.port.with_layer(Layer::Core));
                self.direct(proto, self.shell_transmitter.clone()).await
            }
            PipelineStopVar::Reflect => {
                let reflection = self.reflection.clone()?;
                let mut core = ReflectedCore::status(self.status);
                core.body = self.body.clone();

                let reflected = reflection.make(core, self.traversal.to.clone() );
                self.gravity_transmitter.route(reflected.to_ultra()).await;
                Ok(())
            }
            PipelineStopVar::Call(_) => {
                unimplemented!()
            }
            PipelineStopVar::Point(point) => {
                let point: Point = point.clone().to_resolved(&self.env)?;
                let mut proto = self.proto();
                proto.to(point.to_port().with_layer(Layer::Core));
                self.direct(proto, self.gravity_transmitter.clone()).await
            }
            PipelineStopVar::Err { .. } => {
                unimplemented!()
            }
        }
    }

    async fn direct(&mut self, mut proto: DirectedProto, transmitter: ProtoTransmitter ) -> Result<(), UniErr> {

        match proto.kind.as_ref().unwrap() {
            DirectedKind::Ping => {
                let pong: Wave<Pong> = transmitter.direct(proto).await?;
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
                let mut echoes: Vec<Wave<Echo>> = transmitter.direct(proto).await?;
                if !echoes.is_empty()  {
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