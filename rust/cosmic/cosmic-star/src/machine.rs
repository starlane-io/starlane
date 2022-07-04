use crate::implementation::Implementation;
use crate::star::{Star, StarApi, StarCon, StarRouter, StarSkel, StarTemplate};
use cosmic_api::error::MsgErr;
use cosmic_api::id::id::{Point, ToPoint, ToPort};
use cosmic_api::id::StarKey;
use cosmic_api::log::RootLogger;
use cosmic_api::quota::Timeouts;
use cosmic_api::sys::InterchangeKind;
use cosmic_api::wave::{Agent, HyperWave};
use cosmic_api::{Artifacts, RegistryApi};
use cosmic_hyperlane::{HyperGate, HyperRouter, Hyperway, HyperwayInterchange, InterchangeEntryRouter, TokenAuthenticatorWithRemoteWhitelist};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use dashmap::DashMap;
use tokio::sync::{mpsc, oneshot};

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
    pub implementation: Box<dyn Implementation>,
    pub rx: mpsc::Receiver<MachineCall>
}

impl Machine {
    pub fn new(
        implementation: Box<dyn Implementation>,
        template: MachineTemplate,
    ) -> Result<(), MsgErr> {
        let (tx,rx) = mpsc::channel(32*1024);
        let skel = MachineSkel {
            registry: implementation.registry(),
            artifacts: implementation.artifacts(),
            logger: RootLogger::default(),
            timeouts: Timeouts::default(),
            tx
        };

        let mut stars = HashMap::new();
        let mut gates = Arc::new(DashMap::new());
        for star_template in template.stars {
            let point = star_template.key.clone().to_point();
            let (fabric_tx, mut fabric_rx) = mpsc::channel(32 * 1024);
            let mut builder = implementation.drivers_builder(&star_template.kind);
            let drivers_point = point.push("drivers".to_string()).unwrap();
            let logger = skel.logger.point(drivers_point.clone());
            builder.logger.replace(logger.clone());
            let star_skel = StarSkel::new(star_template, skel.clone(), fabric_tx);
            let drivers = builder.build(drivers_point.to_port(), star_skel.clone())?;
            let star_api = Star::new(star_skel.clone(), drivers);
            stars.insert(point, star_api.clone());

            let logger = logger.push("interchange").unwrap();
            let interchange = HyperwayInterchange::new(Box::new(StarRouter::new(star_api)), logger);
            let mut connect_whitelist = HashSet::new();
            for con in star_template.hyperway {
                match con {
                    StarCon::Receive(key) => {
                        connect_whitelist.insert(key.to_point());
                    }
                    StarCon::Connect(key) => {

                    }
                }
            }

            let auth = TokenAuthenticatorWithRemoteWhitelist::new(
                Agent::HyperUser,
                implementation.token(),
                connect_whitelist,
            );
            let gate = HyperGate::new(
                Box::new(auth),
                Arc::new(interchange),
                logger.point(point.clone()).push("gate").unwrap(),
            );
            gates.insert(InterchangeKind::Star(star_template.key.clone()), gate);
        }
        let entry_router = InterchangeEntryRouter::new(gates);

        let mut machine = Self {
            skel,
            stars: Arc::new(stars),
            entry_router,
            implementation,
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
