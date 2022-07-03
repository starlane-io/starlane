use std::collections::HashMap;
use std::sync::Arc;
use dashmap::DashMap;
use tokio::sync::{mpsc, oneshot};
use cosmic_api::{Artifacts, RegistryApi};
use cosmic_api::error::MsgErr;
use cosmic_api::id::id::{Point, ToPoint, ToPort};
use cosmic_api::id::StarKey;
use cosmic_api::log::RootLogger;
use cosmic_api::quota::Timeouts;
use cosmic_api::wave::HyperWave;
use cosmic_hyperlane::{HyperRouter, Hyperway, HyperwayInterchange};
use crate::implementation::Implementation;
use crate::star::{Star, StarApi, StarRouter, StarSkel, StarTemplate};

#[derive(Clone)]
pub struct MachineSkel {
    pub registry: Arc<dyn RegistryApi>,
    pub artifacts: Arc<dyn Artifacts>,
    pub logger: RootLogger,
    pub timeouts: Timeouts
}




pub struct Machine {
   pub skel: MachineSkel,
   pub stars: Arc<HashMap<Point,StarApi>>,
   pub lanes: Arc<DashMap<StarKey,HyperwayInterchange>>,
   pub implemenation: Box<dyn Implementation>
}

impl Machine {
    pub fn new( skel: MachineSkel, implementation: Box<dyn Implementation>, template: MachineTemplate ) -> Result<(),MsgErr> {
        let mut stars = HashMap::new();
        for star_template in template.stars {
            let point = star_template.key.clone().to_point();
            let (fabric_tx, mut fabric_rx) = mpsc::channel(32*1024);
            let mut builder = implementation.drivers_builder(&star_template.kind);
            let drivers_point = point.push("drivers".to_string()).unwrap();
            let logger = skel.logger.point(drivers_point.clone());
            builder.logger.replace(logger.clone());
            let star_skel = StarSkel::new(star_template, skel.clone(), fabric_tx );
            let drivers = builder.build(drivers_point.to_port(), star_skel.clone())?;
            let star_api = Star::new(star_skel.clone(), drivers );
            stars.insert(point, star_api.clone() );

            let logger = logger.push( "interchange").unwrap();
            let interchange = HyperwayInterchange::new(Box::new(StarRouter::new(star_api)), logger );

            let router = interchange.router();
            let registry = skel.registry.clone();
            tokio::spawn( async move {
               while let Some(wave) =  fabric_rx.recv().await {
                   registry.locate( wave.to() )
                   router.route(wave).await;
               }
            });

/*            for hyperway in star_template.hyperway {

            }

 */
        }


        Ok(())
    }
}


pub enum MachineCall {
   StarConnectTo { star: StarKey, tx: oneshot::Sender<Hyperway> }
}

pub struct MachineTemplate {
    pub stars: Vec<StarTemplate>
}