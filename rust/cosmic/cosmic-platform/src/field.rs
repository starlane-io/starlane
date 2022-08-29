use crate::star::{LayerInjectionRouter, StarSkel};
use crate::{PlatErr, Platform};
use cosmic_api::config::config::bind::{BindConfig, PipelineStepVar, PipelineStopVar, WaveDirection};
use cosmic_api::error::{MsgErr, StatusErr};
use cosmic_api::id::id::{Layer, Point, Port, ToPoint, ToPort, TraversalLayer};
use cosmic_api::id::Traversal;
use cosmic_api::wave::{DirectedKind, DirectedProto, DirectedWave, Echo, Exchanger, Method, Pong, ProtoTransmitter, ProtoTransmitterBuilder, ReflectedAggregate, ReflectedCore, Reflection, UltraWave, Wave, WaveKind};
use cosmic_api::ArtRef;
use std::str::FromStr;
use std::sync::Arc;
use http::Uri;
use cosmic_api::log::{PointLogger, Trackable};
use cosmic_api::parse::model::{MethodScope, PipelineVar};
use cosmic_api::parse::{Env, RegexCapturesResolver};
use cosmic_api::selector::{PayloadBlock, PayloadBlockVar};
use cosmic_api::substance::substance::Substance;
use cosmic_api::util::ToResolved;

pub struct Field<P>
where
    P: Platform,
{
    pub port: Port,
    pub skel: StarSkel<P>,
    pub logger: PointLogger,
    pub transmitter: ProtoTransmitter,
}

impl<P> Field<P>
where
    P: Platform,
{
    pub fn new(point: Point, skel: StarSkel<P>) -> Self {
        let port = point.to_port().with_layer(Layer::Field);
        let logger = skel.logger.point(port.point.clone());
        let router = Arc::new(LayerInjectionRouter::new(skel.clone(), port.clone()));
        let transmitter = ProtoTransmitterBuilder::new(router, skel.exchanger.clone());
        let transmitter = transmitter.build();
        Self {
            port,
            skel,
            logger,
            transmitter,
        }
    }

    async fn bind(&self, directed: &Traversal<DirectedWave>) -> Result<ArtRef<BindConfig>, MsgErr> {
        let record = self.skel.registry.locate(&self.port.point).await.map_err(|e|e.to_cosmic_err())?;
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
        self.transmitter.route(wave).await;
    }

    fn exchanger(&self) -> &Exchanger {
        &self.skel.exchanger
    }

    async fn directed_core_bound(&self, directed: Traversal<DirectedWave>) -> Result<(), MsgErr> {
        let bind = self.bind(&directed).await?;
        let route = bind.select(&directed.payload)?;
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

        // PipeEx will execute itself
        PipeEx::new( self.port.clone(), directed, route.block.clone(), env, self.transmitter.clone(), self.logger.clone() );

        Ok(())
    }
}

pub struct PipeEx {
  pub port: Port,
  pub logger: PointLogger,
  pub env: Env,
  pub reflection: Result<Reflection,MsgErr>,
  pub pipeline: PipelineVar,
  pub transmitter: ProtoTransmitter,
  pub traversal: Traversal<DirectedWave>,

  pub kind: DirectedKind,
  pub method: Method,
  pub uri: Uri,
  pub body: Substance,
  pub status: u16
}

impl PipeEx {
    pub fn new( port: Port, traversal: Traversal<DirectedWave>, pipeline: PipelineVar, env: Env, transmitter: ProtoTransmitter, logger: PointLogger ) {
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
                transmitter,
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
                        let wave = reflection.clone().make(err.as_reflected_core(), self.port.clone() ).to_ultra();
                        self.transmitter.route(wave).await;
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
        proto.track = self.traversal.track();
        proto
    }

    pub async fn execute( &mut self ) -> Result<(),MsgErr> {
        while let Some( segment ) = self.pipeline.consume() {
            self.execute_step(&segment.step)?;
            self.execute_stop(&segment.stop).await?;
        }
        Ok(())
    }


    fn execute_step(&mut self, step: &PipelineStepVar) -> Result<(), MsgErr> {
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

    async fn execute_stop( &mut self, stop: &PipelineStopVar) -> Result<(),MsgErr> {

        match stop {
            PipelineStopVar::Core => {
                let mut proto = self.proto();
                proto.to(self.port.with_layer(Layer::Core));
                self.direct(proto).await
            }
            PipelineStopVar::Reflect => {
                let reflection = self.reflection.clone()?;
                let mut core = ReflectedCore::status(self.status);
                core.body = self.body.clone();
                let reflected = reflection.make(core, self.traversal.to.clone() );
                self.transmitter.route(reflected.to_ultra()).await;
                Ok(())
            }
            PipelineStopVar::Call(_) => {
                unimplemented!()
            }
            PipelineStopVar::Point(point) => {
                let point: Point = point.clone().to_resolved(&self.env)?;
                let mut proto = self.proto();
                proto.to(point.to_port().with_layer(Layer::Core));
                self.direct(proto).await
            }
            PipelineStopVar::Err { .. } => {
                unimplemented!()
            }
        }
    }

    async fn direct( &mut self, proto: DirectedProto ) -> Result<(),MsgErr> {

        match proto.kind.as_ref().unwrap() {
            DirectedKind::Ping => {
                let pong: Wave<Pong> = self.transmitter.direct(proto).await?;
                self.status = pong.core.status.as_u16();
                if pong.core.status.is_success() {
                    self.body = pong.core.body.clone();
                    Ok(())
                } else {
                    Err(pong.core.to_err()?)
                }
            }
            DirectedKind::Ripple => {
                // this should be a single echo since in traversal it is only going to a single target
                let mut echoes: Vec<Wave<Echo>> = self.transmitter.direct(proto).await?;
                if !echoes.is_empty()  {
                    let echo = echoes.remove(0);
                    self.status = echo.core.status.as_u16();
                    if echo.core.status.is_success() {
                        self.body = echo.core.body.clone();
                        Ok(())
                    } else {
                        Err(echo.core.to_err()?)
                    }
                } else {
                    Ok(())
                }
            }
            DirectedKind::Signal => {
                Ok(())
            }
        }
    }

}