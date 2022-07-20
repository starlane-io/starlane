use crate::{PlatErr, Platform,  RegistryApi};
use crate::star::{Star, StarApi, StarCon, StarRouter, StarSkel, StarTemplate};
use cosmic_api::error::MsgErr;
use cosmic_api::id::id::{Point, ToPoint, ToPort};
use cosmic_api::id::{ConstellationName, StarHandle, StarKey, StarSub};
use cosmic_api::log::RootLogger;
use cosmic_api::quota::Timeouts;
use cosmic_api::sys::{EntryReq, InterchangeKind};
use cosmic_api::wave::{Agent, HyperWave, UltraWave};
use cosmic_api::{ArtifactApi};
use cosmic_hyperlane::{HyperClient, HyperGate, HyperRouter, Hyperway, HyperwayIn, HyperwayInterchange, InterchangeEntryRouter, LocalClientConnectionFactory, TokenAuthenticatorWithRemoteWhitelist};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use dashmap::DashMap;
use tokio::sync::{mpsc, oneshot};
use cosmic_api::substance::substance::Substance;

#[derive(Clone)]
pub struct MachineSkel<P> where P: Platform{
    pub platform: P,
    pub registry: Arc<dyn RegistryApi<P>>,
    pub artifacts: ArtifactApi,
    pub logger: RootLogger,
    pub timeouts: Timeouts,
    pub tx: mpsc::Sender<MachineCall>
}

pub struct Machine<P> where P: Platform+'static{
    pub skel: MachineSkel<P>,
    pub stars: Arc<HashMap<Point, StarApi>>,
    pub entry_router: InterchangeEntryRouter,
    pub interchanges: HashMap<StarKey,Arc<HyperwayInterchange>>,
    pub rx: mpsc::Receiver<MachineCall>
}

impl <P> Machine<P> where P: Platform+'static{
    pub fn new(
        platform: P
    ) -> Result<(), P::Err> {
        let template = platform.machine_template();

        let pool = Arc::new(platform.create_registry_context(template.star_set())?);

        let (tx,rx) = mpsc::channel(32*1024);
        let skel = MachineSkel {
            registry: platform.global_registry(pool.clone()),
            artifacts: platform.artifact_hub(),
            logger: RootLogger::default(),
            timeouts: Timeouts::default(),
            platform,
            tx
        };

        let mut stars = HashMap::new();
        let mut gates = Arc::new(DashMap::new());
        let mut clients = vec![];
        let mut interchanges = HashMap::new();
        for star_template in template.stars {
            let star_point = star_template.key.clone().to_point();
            let (fabric_tx, mut fabric_rx) = mpsc::channel(32 * 1024);
            let mut builder = skel.platform.drivers_builder(&star_template.kind);
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
                skel.platform.token(),
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
                auth: Box::new(Substance::Token(skel.platform.token())),
                remote: Some(from.clone())
            };

            let logger = skel.logger.point(from.clone());
            let factory = LocalClientConnectionFactory::new( entry_req, entry_router.clone() );
            let hyperway = HyperClient::new( Agent::HyperUser, to.to_point(), Box::new(factory), logger)?;
            interchanges.get(&StarKey::try_from(from).unwrap()).unwrap().add( hyperway );
        }

        skel.platform.start_services(& mut entry_router);

        let mut machine = Self {
            skel,
            stars: Arc::new(stars),
            entry_router,
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

impl MachineTemplate {
    pub fn star_set(&self) -> HashSet<StarKey> {
        let mut rtn = HashSet::new();
        for star in self.stars.iter() {
            rtn.insert( star.key.clone() );
        }
        rtn
    }
}

impl Default for MachineTemplate {
    fn default() -> Self {
        let constellation = "central".to_string();

        let mut central= StarTemplate::new (StarKey::central(), StarSub::Central);
        let mut nexus = StarTemplate::new (StarKey::new(&constellation, &StarHandle::name("nexus")), StarSub::Nexus);
        let mut supe= StarTemplate::new (StarKey::new(&constellation, &StarHandle::name("super")), StarSub::Super);
        let mut maelstrom= StarTemplate::new (StarKey::new(&constellation, &StarHandle::name("maelstrom")), StarSub::Maelstrom);
        let mut scribe= StarTemplate::new (StarKey::new(&constellation, &StarHandle::name("scribe")), StarSub::Scribe);
        let mut jump = StarTemplate::new (StarKey::new(&constellation, &StarHandle::name("jump")), StarSub::Jump);
        let mut fold = StarTemplate::new (StarKey::new(&constellation, &StarHandle::name("fold")), StarSub::Fold);

        nexus.receive(central.key.clone() );
        nexus.receive(supe.key.clone() );
        nexus.receive(maelstrom.key.clone() );
        nexus.receive(scribe.key.clone() );
        nexus.receive(jump.key.clone() );
        nexus.receive(fold.key.clone() );

        central.connect(nexus.key.clone());
        supe.connect(nexus.key.clone());
        maelstrom.connect(nexus.key.clone());
        scribe.connect(nexus.key.clone());
        jump.connect(nexus.key.clone());
        fold.connect(nexus.key.clone());

        let mut stars = vec![];
        stars.push(central);
        stars.push(nexus);
        stars.push(supe);
        stars.push(maelstrom);
        stars.push(scribe);
        stars.push(jump);
        stars.push(fold);

        Self {
            stars
        }
    }
}
