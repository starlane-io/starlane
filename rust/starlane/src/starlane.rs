use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::convert::TryInto;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

use futures::future::join_all;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::oneshot::error::RecvError;

use api::SpaceApi;

use crate::error::Error;
use crate::frame::{ChildManagerResourceAction, Frame, Reply, SimpleReply, StarMessagePayload};
use crate::keys::ResourceKey;
use crate::lane::{ConnectionInfo, ConnectionKind, Lane, LocalTunnelConnector};
use crate::layout::ConstellationLayout;
use crate::logger::{Flags, Logger};
use crate::message::{Fail, ProtoStarMessage};
use crate::proto::{local_tunnels, ProtoStar, ProtoStarController, ProtoStarEvolution, ProtoTunnel};
use crate::provision::Provisioner;
use crate::resource::{AddressCreationSrc, AssignResourceStateSrc, KeyCreationSrc, ResourceAddress, ResourceArchetype, ResourceCreate, ResourceKind, ResourceRecord};
use crate::resource::space::SpaceState;
use crate::star::{Request, Star, StarCommand, StarController, StarKey, StarName};
use crate::template::{ConstellationData, ConstellationTemplate, StarKeyIndexTemplate, StarKeySubgraphTemplate, StarKeyTemplate};
use crate::starlane::api::StarlaneApi;
use crate::core::CoreRunner;
use crate::star::variant::{StarVariantFactory, StarVariantFactoryDefault};

pub mod api;

lazy_static!{
   pub static ref DATA_DIR: Mutex<String> = Mutex::new("data".to_string());
}

pub struct Starlane
{
    pub tx: mpsc::Sender<StarlaneCommand>,
    rx: mpsc::Receiver<StarlaneCommand>,
    star_controllers: HashMap<StarKey,StarController>,
    star_names: HashMap<StarName,StarKey>,
    star_manager_factory: Arc<dyn StarVariantFactory>,
//    star_core_ext_factory: Arc<dyn StarCoreExtFactory>,
    core_runner: Arc<CoreRunner>,
    constellation_names: HashSet<String>,
    pub logger: Logger,
    pub flags: Flags
}

impl Starlane
{
    pub fn new()->Result<Self,Error>
    {
        let (tx, rx) = mpsc::channel(32);
        Ok(Starlane{
            star_controllers: HashMap::new(),
            star_names: HashMap::new(),
            constellation_names: HashSet::new(),
            tx: tx,
            rx: rx,
            star_manager_factory: Arc::new( StarVariantFactoryDefault {} ),
//            star_core_ext_factory: Arc::new(ExampleStarCoreExtFactory::new() ),
            core_runner: Arc::new(CoreRunner::new()?),
            logger: Logger::new(),
            flags: Flags::new()
        })
    }

    pub async fn run(&mut self)
    {
        while let Option::Some(command) = self.rx.recv().await
        {
            match command
            {
                StarlaneCommand::Connect(command)=> {
/*                    if self.stars.contains_key(&command.key)
                    {

                    }
                    else {
                        command.oneshot.send( Err(format!("could not find host address for star: {}", &command.key).into()) );
                    }
 */
                    unimplemented!()
                }
                StarlaneCommand::ConstellationCreate(command) => {
                    let result = self.constellation_create(command.template, command.data, command.name ).await;
                    command.tx.send(result);
                }
                StarlaneCommand::StarControlRequestByName(request) => {
                   if let Option::Some(key) = self.star_names.get(&request.name)
                   {
                       if let Option::Some(ctrl) = self.star_controllers.get(key)
                       {
                           request.tx.send(StarlaneApi::new(ctrl.star_tx.clone()));
                       }
                   }
                }
                StarlaneCommand::Destroy => {
                    println!("closing rx");
                    self.rx.close();
                }
                _ => {}
            }
        }
    }

    async fn lookup_star_address( &self, key: &StarKey )->Result<StarAddress,Error>
    {
        if self.star_controllers.contains_key(key)
        {
            Ok(StarAddress::Local)
        }
        else {
            Err(format!("could not find address for starkey: {}", key.to_string()).into() )
        }
    }

    async fn provision_link(&mut self, template: ConstellationTemplate, mut data: ConstellationData, connection_info: ConnectionInfo) ->Result<(),Error>
    {
        let link = template.get_star("link".to_string() );
        if link.is_none()
        {
            return Err("link is not present in the constellation template".into());
        }

        let link = link.unwrap().clone();
        let (mut evolve_tx,mut evolve_rx) = oneshot::channel();
        let (proto_star, star_ctrl) = ProtoStar::new(Option::None, link.kind.clone(), self.star_manager_factory.clone(), self.core_runner.clone(), self.flags.clone(), self.logger.clone() );

        println!("created proto star: {:?}", &link.kind);

        let starlane_ctrl = self.tx.clone();
        tokio::spawn( async move {
            let star = proto_star.evolve().await;
            if let Ok(star) = star
            {
                data.exclude_handles.insert("link".to_string() );
                data.subgraphs.insert("client".to_string(), star.star_key().subgraph.clone() );

                let (tx,rx) = oneshot::channel();
                starlane_ctrl.send( StarlaneCommand::ConstellationCreate(
                    ConstellationCreate {
                        name: Option::None,
                        template: template,
                        data: data,
                        tx: tx
                    }
                ));

                evolve_tx.send( ProtoStarEvolution{ star: star.star_key().clone(), controller: StarController { star_tx: star.star_tx() } });
                
                star.run().await;
            }
            else {
                eprintln!("experienced serious error could not evolve the proto_star");
            }
        } );

        match connection_info.kind
        {
            ConnectionKind::Starlane => {
                let high_star_ctrl = star_ctrl.clone();
                let low_star_ctrl =
                    {
                        let low_star_ctrl = self.star_controllers.get_mut(&connection_info.gateway);
                        match low_star_ctrl
                        {
                            None => {
                                return Err(format!("lane cannot construct. missing second star key: {}", &connection_info.gateway.to_string()).into())
                            }
                            Some(low_star_ctrl) => {low_star_ctrl.clone()}
                        }
                    };

                self.add_local_lane_ctrl(Option::None, Option::Some(connection_info.gateway.clone()), high_star_ctrl,low_star_ctrl).await?;

            }
            ConnectionKind::Url(_) => {
                eprintln!("not supported yet")
            }
        }


        if let Ok(evolve) = evolve_rx.await
        {
            self.star_controllers.insert(evolve.star,evolve.controller);
        }
        else {
           eprintln!("got an error message on protostarevolution")
        }

        // now we need to create the lane to the desired gateway which is what the Link is all about

        Ok(())
    }

    async fn constellation_create(&mut self, template: ConstellationTemplate, data: ConstellationData, name: Option<String>) ->Result<(),Error>
    {
        if name.is_some() && self.constellation_names.contains(name.as_ref().unwrap())
        {
            return Err(format!("a constellation named: {} already exists!", name.as_ref().unwrap()).into() );
        }

        let mut evolve_rxs = vec!();
        for star_template in template.stars.clone()
        {
            if let Some(handle) = &star_template.handle
            {
                if data.exclude_handles.contains(handle )
                {
                    println!("skipping handle: {}", handle);
                    continue;
                }
            }

            let star_key = star_template.key.create(&data)?;
            let (mut evolve_tx,mut evolve_rx) = oneshot::channel();
            evolve_rxs.push(evolve_rx );

            let (proto_star, star_ctrl) = ProtoStar::new(Option::Some(star_key.clone()), star_template.kind.clone(), self.star_manager_factory.clone(), self.core_runner.clone(), self.flags.clone(), self.logger.clone() );
            self.star_controllers.insert(star_key.clone(), star_ctrl.clone() );
            if name.is_some() && star_template.handle.is_some()
            {
                let name = StarName{
                    constellation: name.as_ref().unwrap().clone(),
                    star: star_template.handle.as_ref().unwrap().clone()
                };
                self.star_names.insert( name, star_key.clone() );
            }
            println!("created proto star: {:?}", &star_template.kind);

            tokio::spawn( async move {
                let star = proto_star.evolve().await;
                if let Ok(star) = star
                {
                    let key = star.star_key().clone();
                    let star_tx= star.star_tx();
                    tokio::spawn( async move {
                        star.run().await;
                    });
                    evolve_tx.send( ProtoStarEvolution{
                        star: key,
                        controller: StarController{
                            star_tx: star_tx
                        }
                    });
                    println!("created star: {:?} key: {}", &star_template.kind, star_key.to_string());
                }
                else {
                    eprintln!("experienced serious error could not evolve the proto_star");
                }
            } );
        }

        // now make the LANES
        for star_template in &template.stars
        {
            for lane in &star_template.lanes
            {
                let local = star_template.key.create(&data)?;
                let second = lane.star.create(&data)?;

                self.add_local_lane(local, second ).await?;
            }
        }


        // announce that the constellations is now complete
        for star_template in &template.stars
        {
            if let Option::Some(star_ctrl) = self.star_controllers.get_mut(&star_template.key.create(&data)? )
            {
                star_ctrl.star_tx.send(StarCommand::ConstellationConstructionComplete).await;
            }
        }



        let evolutions = join_all(evolve_rxs).await;

        for evolve in evolutions
        {
            if let Ok(evolve) = evolve
            {
                evolve.controller.star_tx.send(StarCommand::Init).await;
                self.star_controllers.insert(evolve.star, evolve.controller);
            }
            else if let Err(error) = evolve
            {
               return Err(error.to_string().into())
            }
        }

        Ok(())
    }

    async fn add_local_lane(&mut self, local: StarKey, second: StarKey ) ->Result<(),Error>
    {
        let (high,low) = StarKey::sort(local,second)?;
        let high_star_ctrl =
        {
            let high_star_ctrl = self.star_controllers.get_mut(&high);
            match high_star_ctrl
            {
                None => {
                    return Err(format!("lane cannot construct. missing local star key: {}", high.to_string()).into())
                }
                Some(high_star_ctrl) => {high_star_ctrl.clone()}
            }
        };

        let low_star_ctrl =
        {
            let low_star_ctrl = self.star_controllers.get_mut(&low);
            match low_star_ctrl
            {
                None => {
                    return Err(format!("lane cannot construct. missing second star key: {}", low.to_string()).into())
                }
                Some(low_star_ctrl) => {low_star_ctrl.clone()}
            }
        };
        self.add_local_lane_ctrl(Option::Some(high), Option::Some(low), high_star_ctrl,low_star_ctrl).await
    }


    async fn add_local_lane_ctrl(&mut self, high: Option<StarKey>, low: Option<StarKey>, high_star_ctrl: StarController, low_star_ctrl: StarController ) ->Result<(),Error>

    {
        let high_lane= Lane::new(low).await;
        let low_lane = Lane::new(high).await;
        let connector = LocalTunnelConnector::new(&high_lane,&low_lane).await?;
        high_star_ctrl.star_tx.send(StarCommand::AddLane(high_lane)).await?;
        low_star_ctrl.star_tx.send(StarCommand::AddLane(low_lane)).await?;
        high_star_ctrl.star_tx.send( StarCommand::AddConnectorController(connector)).await?;

        Ok(())
    }


}

pub enum StarlaneCommand
{
    Connect(ConnectCommand),
    ConstellationCreate(ConstellationCreate),
    StarControlRequestByKey(StarControlRequestByKey),
    StarControlRequestByName(StarControlRequestByName),
    Destroy
}

pub struct StarControlRequestByKey
{
    pub star: StarKey,
    pub tx: oneshot::Sender<StarlaneApi>
}

pub struct StarControlRequestByName
{
    pub name: StarName,
    pub tx: oneshot::Sender<StarlaneApi>
}

impl StarControlRequestByName
{
    pub fn new( constellation: String, star: String )->(Self,oneshot::Receiver<StarlaneApi>)
    {
        let (tx,rx) = oneshot::channel();
        (StarControlRequestByName{
            name: StarName {
                constellation: constellation,
                star: star
            },
            tx: tx
        },rx)
    }
}

pub struct ConstellationCreate
{
    name: Option<String>,
    template: ConstellationTemplate,
    data: ConstellationData,
    tx: oneshot::Sender<Result<(),Error>>
}

pub struct ConnectCommand
{
    pub key: StarKey,
    pub oneshot: oneshot::Sender<Result<StarAddress,Error>>
}

impl ConnectCommand
{
    pub fn new( key: StarKey, oneshot: oneshot::Sender<Result<StarAddress,Error>>)->Self
    {
        ConnectCommand {
            key: key,
            oneshot: oneshot
        }
    }
}

impl ConstellationCreate
{
    pub fn new(template: ConstellationTemplate, data: ConstellationData, name: Option<String> )->(Self,oneshot::Receiver<Result<(),Error>>)
    {
        let (tx,rx)= oneshot::channel();
        (ConstellationCreate {
            name: name,
            template: template,
            data: data,
            tx: tx
        }, rx)
    }
}


pub enum StarAddress
{
    Local
}



#[cfg(test)]
mod test
{
    use std::str::FromStr;
    use std::sync::Arc;

    use tokio::runtime::Runtime;
    use tokio::sync::oneshot::error::RecvError;
    use tokio::time::Duration;
    use tokio::time::timeout;

    use crate::artifact::{ArtifactKind, ArtifactLocation};
    use crate::error::Error;
    use crate::keys::{SpaceKey, SubSpaceKey, UserKey};
    use crate::logger::{Flag, Flags, Log, LogAggregate, ProtoStarLog, ProtoStarLogPayload, StarFlag, StarLog, StarLogPayload};
    use crate::names::Name;
    use crate::permissions::Authentication;
    use crate::resource::{Labels, ResourceAddress};
    use crate::space::CreateAppControllerFail;
    use crate::star::{StarController, StarInfo, StarKey, StarKind};
    use crate::starlane::{ConstellationCreate, StarControlRequestByName, Starlane, StarlaneCommand};
    use crate::template::{ConstellationData, ConstellationTemplate};
    use crate::starlane::api::SubSpaceApi;
    use crate::message::Fail;
    use std::fs;
    use std::convert::TryInto;

    #[test]
    pub fn starlane()
    {
        let data_dir = "tmp/data";
        fs::remove_dir_all(data_dir ).unwrap();
        std::env::set_var("STARLANE_DATA", data_dir );


        let rt = Runtime::new().unwrap();
        rt.block_on(async {

            let mut starlane = Starlane::new().unwrap();
            starlane.flags.on(Flag::Star(StarFlag::DiagnosePledge) );
            let mut agg = LogAggregate::new();
            agg.watch(starlane.logger.clone()).await;
            let tx = starlane.tx.clone();

            let handle = tokio::spawn( async move {
                starlane.run().await;
            } );

            {
                let (command, mut rx) = ConstellationCreate::new(ConstellationTemplate::new_standalone(), ConstellationData::new(), Option::Some("standalone".to_owned()));
                tx.send(StarlaneCommand::ConstellationCreate(command)).await;
                let result = rx.await;
                match result {
                    Ok(result) => {
                        match result {
                            Ok(_) => {}
                            Err(e) => {
                                println!("error: {}", e)
                            }
                        }
                    }
                    Err(e) => {
                        println!("error: {}", e)
                    }
                }
            }

            tokio::time::sleep(Duration::from_secs(1)).await;

            let starlane_api = {
                let (request,rx) = StarControlRequestByName::new("standalone".to_owned(), "mesh".to_owned());
                tx.send(StarlaneCommand::StarControlRequestByName(request)).await;
                timeout(Duration::from_millis(10), rx).await.unwrap().unwrap()
            };

            let sub_space_api = match starlane_api.get_sub_space(ResourceAddress::from_str("hyperspace:default::<SubSpace>").unwrap().into() ).await
            {
                Ok(api) => api,
                Err(err) => {
eprintln!("{}",err.to_string());
                    panic!(err)
                }
            };

            let file_api = sub_space_api.create_file_system("website").unwrap().submit().await.unwrap();
            file_api.create_file_from_string(&"/README.txt".try_into().unwrap(), "The rain in Spain falls mostly on the plain.".to_string() ).unwrap().submit().await.unwrap();
            file_api.create_file_from_string(&"/images/not-really-an-image.png".try_into().unwrap(), "This is not really an image....".to_string() ).unwrap().submit().await.unwrap();


            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
            }


//            assert_eq!(central_ctrl.diagnose_handlers_satisfaction().await.unwrap(),crate::star::pledge::Satisfaction::Ok)

        });

    }
}
