use crate::platform::Platform;
use crate::star::{Star, StarApi, StarCon, StarRouter, StarSkel, StarTemplate};
use cosmic_api::error::MsgErr;
use cosmic_api::id::id::{Point, ToPoint, ToPort};
use cosmic_api::id::StarKey;
use cosmic_api::log::RootLogger;
use cosmic_api::quota::Timeouts;
use cosmic_api::sys::InterchangeKind;
use cosmic_api::wave::{Agent, HyperWave};
use cosmic_api::{Artifacts, RegistryApi};
use cosmic_hyperlane::{HyperClient, HyperGate, HyperRouter, Hyperway, HyperwayInterchange, InterchangeEntryRouter, LocalClientConnectionFactory, TokenAuthenticatorWithRemoteWhitelist};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use dashmap::DashMap;
use tokio::sync::{mpsc, oneshot};
use cosmic_api::substance::substance::Substance;
use cosmic_api::wave::SysMethod::EntryReq;

#[derive(Clone)]
pub struct MachineSkel {
    pub registry: Arc<dyn RegistryApi>,
    pub artifacts: Arc<dyn Artifacts>,
    pub logger: RootLogger,
    pub timeouts: Timeouts,
    pub tx: mpsc::Sender<MachineCall>
}

pub struct Machine {
    pub skel: MachineSkel,
    pub stars: Arc<HashMap<Point, StarApi>>,
    pub entry_router: InterchangeEntryRouter,
    pub platform: Box<dyn Platform>,
    pub rx: mpsc::Receiver<MachineCall>
}

impl Machine {
    pub fn new(
        platform: Box<dyn Platform>,
        template: MachineTemplate,
    ) -> Result<(), MsgErr> {
        let (tx,rx) = mpsc::channel(32*1024);
        let skel = MachineSkel {
            registry: platform.registry(),
            artifacts: platform.artifacts(),
            logger: RootLogger::default(),
            timeouts: Timeouts::default(),
            tx
        };

        let mut stars = HashMap::new();
        let mut gates = Arc::new(DashMap::new());
        let mut clients = vec![];
        for star_template in template.stars {
            let star_point = star_template.key.clone().to_point();
            let (fabric_tx, mut fabric_rx) = mpsc::channel(32 * 1024);
            let mut builder = platform.drivers_builder(&star_template.kind);
            let drivers_point = star_point.push("drivers".to_string()).unwrap();
            let logger = skel.logger.point(drivers_point.clone());
            builder.logger.replace(logger.clone());
            let star_skel = StarSkel::new(star_template.clone(), skel.clone(), fabric_tx);
            let drivers = builder.build(drivers_point.to_port(), star_skel.clone())?;
            let star_api = Star::new(star_skel.clone(), drivers);
            stars.insert(star_point.clone(), star_api.clone());

            let interchange = HyperwayInterchange::new(Box::new(StarRouter::new(star_api)), logger.push("interchange").unwrap());
            let mut connect_whitelist = HashSet::new();
            for con in &star_template.hyperway {
                match con {
                    StarCon::Receive(key) => {
                        connect_whitelist.insert(key.clone().to_poin());
                    }
                    StarCon::Connect(key) => {
                        clients.push( (star_point.clone(),key.clone()) )
                    }
                }
            }

            let auth = TokenAuthenticatorWithRemoteWhitelist::new(
                Agent::HyperUser,
                platform.token(),
                connect_whitelist,
            );
            let gate = HyperGate::new(
                Box::new(auth),
                Arc::new(interchange),
                logger.point(star_point.clone()).push("gate").unwrap(),
            );
            gates.insert(InterchangeKind::Star(star_template.key.clone()), gate);
        }
        let entry_router = InterchangeEntryRouter::new(gates);

        // now lets make the clients
        for (from,to) in clients {
            let entry_req = EntryReq {
                interchange: InterchangeKind::Star(to.clone()),
                auth: Box::new(Substance::Token(platform.token())),
                remote: Some(from.clone())
            };

            let logger = skel.logger.point(star_point);
            let factory = LocalClientConnectionFactory::new( entry_req, entry_router.clone() );
            let hyperway = HyperClient::new( Agent::HyperUser, to.to_point(), Box::new(factory), logger)?;
            entry_router.add( InterchangeKind::Star(StarKey::try_from(from).unwrap()), hyperway );
        }

        let mut machine = Self {
            skel,
            stars: Arc::new(stars),
            entry_router,
            platform: platform,
            rx
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
