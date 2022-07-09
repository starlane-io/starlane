use crate::Platform;
use crate::star::{Star, StarApi, StarCon, StarRouter, StarSkel, StarTemplate};
use cosmic_api::error::MsgErr;
use cosmic_api::id::id::{Point, ToPoint, ToPort};
use cosmic_api::id::StarKey;
use cosmic_api::log::RootLogger;
use cosmic_api::quota::Timeouts;
use cosmic_api::sys::{EntryReq, InterchangeKind};
use cosmic_api::wave::{Agent, HyperWave, UltraWave};
use cosmic_api::{ArtifactApi, PlatformErr, RegistryApi, RegistryErr};
use cosmic_hyperlane::{HyperClient, HyperGate, HyperRouter, Hyperway, HyperwayIn, HyperwayInterchange, InterchangeEntryRouter, LocalClientConnectionFactory, TokenAuthenticatorWithRemoteWhitelist};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use dashmap::DashMap;
use tokio::sync::{mpsc, oneshot};
use cosmic_api::substance::substance::Substance;

#[derive(Clone)]
pub struct MachineSkel<E> where E: PlatformErr+'static {
    pub registry: RegistryErr<E>,
    pub artifacts: Arc<dyn ArtifactApi>,
    pub logger: RootLogger,
    pub timeouts: Timeouts,
    pub tx: mpsc::Sender<MachineCall>
}

pub struct Machine<E> where E: PlatformErr+'static {
    pub skel: MachineSkel<E>,
    pub stars: Arc<HashMap<Point, StarApi>>,
    pub entry_router: InterchangeEntryRouter,
    pub interchanges: HashMap<StarKey,Arc<HyperwayInterchange>>,
    pub platform: Box<dyn Platform<E>>,
    pub rx: mpsc::Receiver<MachineCall>
}

impl <E> Machine<E> where E: PlatformErr+'static{
    pub fn new(
        platform: Box<dyn Platform<E>>,
        template: MachineTemplate,
    ) -> Result<(), MsgErr> {
        let (tx,rx) = mpsc::channel(32*1024);
        let skel = MachineSkel {
            registry: RegistryErr::new(platform.registry()),
            artifacts: platform.artifacts(),
            logger: RootLogger::default(),
            timeouts: Timeouts::default(),
            tx
        };

        let mut stars = HashMap::new();
        let mut gates = Arc::new(DashMap::new());
        let mut clients = vec![];
        let mut interchanges = HashMap::new();
        for star_template in template.stars {
            let star_point = star_template.key.clone().to_point();
            let (fabric_tx, mut fabric_rx) = mpsc::channel(32 * 1024);
            let mut builder = platform.drivers_builder(&star_template.kind);
            let drivers_point = star_point.push("drivers".to_string()).unwrap();
            let logger = skel.logger.point(drivers_point.clone());
            builder.logger.replace(logger.clone());
            let star_skel = StarSkel::new(star_template.clone(), skel.clone(), builder.kinds() );
            let drivers = builder.build(drivers_point.to_port(), star_skel.clone())?;
            let star_api = Star::new(star_skel.clone(), drivers, fabric_tx )?;
            stars.insert(star_point.clone(), star_api.clone());

            let interchange = Arc::new(HyperwayInterchange::new(Box::new(StarRouter::new(star_api)), logger.push("interchange").unwrap()));
            interchanges.insert( star_template.key.clone(), interchange.clone() );
            let mut connect_whitelist = HashSet::new();
            for con in &star_template.hyperway {
                match con {
                    StarCon::Receive(key) => {
                        connect_whitelist.insert(key.clone().to_point());
                    }
                    StarCon::Connect(key) => {
                        clients.push( (star_point.clone(),key.clone()) )
                    }
                }
            }

            {
                let router = interchange.router();
                tokio::spawn( async move {
                   while let Some(wave) = fabric_rx.recv().await {
                       router.route(wave).await;
                   }
                });
            }

            let auth = TokenAuthenticatorWithRemoteWhitelist::new(
                Agent::HyperUser,
                platform.token(),
                connect_whitelist,
            );

            let gate = HyperGate::new(
                Box::new(auth),
                interchange,
                logger.point(star_point.clone()).push("gate").unwrap(),
            );

            gates.insert(InterchangeKind::Star(star_template.key.clone()), gate);
        }

        let mut entry_router = InterchangeEntryRouter::new(gates);

        // now lets make the clients
        for (from,to) in clients {
            let entry_req = EntryReq {
                interchange: InterchangeKind::Star(to.clone()),
                auth: Box::new(Substance::Token(platform.token())),
                remote: Some(from.clone())
            };

            let logger = skel.logger.point(from.clone());
            let factory = LocalClientConnectionFactory::new( entry_req, entry_router.clone() );
            let hyperway = HyperClient::new( Agent::HyperUser, to.to_point(), Box::new(factory), logger)?;
            interchanges.get(&StarKey::try_from(from).unwrap()).unwrap().add( hyperway );
        }

        platform.start_services(& mut entry_router);

        let mut machine = Self {
            skel,
            stars: Arc::new(stars),
            entry_router,
            platform,
            rx,
            interchanges
        };

        machine.start();

        Ok(())
    }

    pub fn start(mut self) {
        tokio::spawn( async move {
            while let Some(call) = self.rx.recv().await {

            }
        });
    }
}

pub enum MachineCall {
    StarConnectTo {
        star: StarKey,
        tx: oneshot::Sender<Hyperway>,
    },
}

pub struct MachineTemplate {
    pub stars: Vec<StarTemplate>,
}
