use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

use futures::future::join_all;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::oneshot::error::RecvError;

use crate::error::Error;
use crate::frame::Frame;
use crate::lane::{ConnectionInfo, ConnectionKind, Lane, LocalTunnelConnector};
use crate::layout::ConstellationLayout;
use crate::proto::{local_tunnels, ProtoStar, ProtoStarController, ProtoStarEvolution, ProtoTunnel};
use crate::provision::Provisioner;
use crate::star::{Star, StarCommand, StarController, StarKey, StarManagerFactory, StarManagerFactoryDefault};
use crate::template::{ConstellationData, ConstellationTemplate, StarKeyIndexTemplate, StarKeySubgraphTemplate, StarKeyTemplate};

pub struct Starlane
{
    pub tx: mpsc::Sender<StarlaneCommand>,
    rx: mpsc::Receiver<StarlaneCommand>,
    star_controllers: HashMap<StarKey,StarController>,
    star_core_provider: Arc<dyn StarManagerFactory>
}

impl Starlane
{
    pub fn new()->Self
    {
        let (tx, rx) = mpsc::channel(32);
        Starlane{
            star_controllers: HashMap::new(),
            tx: tx,
            rx: rx,
            star_core_provider: Arc::new( StarManagerFactoryDefault{} )
        }
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
                StarlaneCommand::ProvisionConstellation(command) => {
                    let result = self.provision_constellation(command.template, command.data).await;
                    command.oneshot.send(result);
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
            Err(format!("could not find address for starkey: {}", key).into() )
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
        let (proto_star, star_ctrl) = ProtoStar::new(Option::None, link.kind.clone(), evolve_tx, self.star_core_provider.clone() );

        println!("created proto star: {:?}", &link.kind);

        let starlane_ctrl = self.tx.clone();
        tokio::spawn( async move {
            let star = proto_star.evolve().await;
            if let Ok(star) = star
            {
                data.exclude_handles.insert("link".to_string() );
                data.subgraphs.insert("client".to_string(), star.star_key.subgraph.clone() );

                let (tx,rx) = oneshot::channel();
                starlane_ctrl.send( StarlaneCommand::ProvisionConstellation(
                    ProvisionConstellationCommand{
                        template: template,
                        data: data,
                        oneshot: tx
                    }
                ));

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
                                return Err(format!("lane cannot construct. missing second star key: {}", &connection_info.gateway).into())
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

    async fn provision_constellation(&mut self, template: ConstellationTemplate, data: ConstellationData ) ->Result<(),Error>
    {
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

            let (proto_star, star_ctrl) = ProtoStar::new(Option::Some(star_key.clone()), star_template.kind.clone(), evolve_tx, self.star_core_provider.clone() );
            self.star_controllers.insert(star_key.clone(), star_ctrl.clone() );
            println!("created proto star: {:?}", &star_template.kind);

            tokio::spawn( async move {
                println!("evolving proto star..." );
                let star = proto_star.evolve().await;
                if let Ok(star) = star
                {
                    println!("created star: {:?} key: {}", &star_template.kind, star_key);
                    star.run().await;
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

        let evolutions = join_all(evolve_rxs).await;

        for evolve in evolutions
        {
            if let Ok(evolve) = evolve
            {
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
                    return Err(format!("lane cannot construct. missing local star key: {}", high).into())
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
                    return Err(format!("lane cannot construct. missing second star key: {}", low).into())
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
        high_star_ctrl.command_tx.send(StarCommand::AddLane(high_lane))?;
        low_star_ctrl.command_tx.send(StarCommand::AddLane(low_lane))?;
        high_star_ctrl.command_tx.send( StarCommand::AddConnectorController(connector))?;

        Ok(())
    }


}

pub enum StarlaneCommand
{
    Connect(ConnectCommand),
    ProvisionConstellation(ProvisionConstellationCommand),
    RequestStarControlByStarKey(StarControlRequest),
    Destroy
}

pub struct StarControlRequest
{
    pub star: StarKey,
    pub rtn: oneshot::Sender<StarController>
}

pub struct ProvisionConstellationCommand
{
    template: ConstellationTemplate,
    data: ConstellationData,
    oneshot: oneshot::Sender<Result<(),Error>>
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

impl ProvisionConstellationCommand
{
    pub fn new(template: ConstellationTemplate, data: ConstellationData )->(Self,oneshot::Receiver<Result<(),Error>>)
    {
        let (tx,rx)= oneshot::channel();
        (ProvisionConstellationCommand{
            template: template,
            data: data,
            oneshot: tx
        },rx)
    }
}


pub enum StarAddress
{
    Local
}



#[cfg(test)]
mod test
{
    use tokio::runtime::Runtime;
    use tokio::sync::oneshot::error::RecvError;
    use tokio::time::Duration;

    use crate::error::Error;
    use crate::starlane::{ProvisionConstellationCommand, Starlane, StarlaneCommand};
    use crate::template::{ConstellationData, ConstellationTemplate};

    #[test]
    pub fn starlane()
    {

        let rt = Runtime::new().unwrap();
        rt.block_on(async {

            let mut starlane = Starlane::new();
            let tx = starlane.tx.clone();

            let handle = tokio::spawn( async move {
                starlane.run().await;
            } );

            {
                let (command, mut rx) = ProvisionConstellationCommand::new(ConstellationTemplate::new_standalone(), ConstellationData::new());
                tx.send(StarlaneCommand::ProvisionConstellation(command)).await;
                let result = rx.await;
                match result {
                    Ok(result) => {
                        match result {
                            Ok(_) => {println!("template ok.")}
                            Err(e) => {println!("{}", e)}
                        }
                    }
                    Err(e) => {println!("{}", e)}
                }
            }
            tokio::time::sleep(Duration::from_secs(10)).await;

            println!("sending Destroy command.");
            tx.send(StarlaneCommand::Destroy ).await;

            handle.await;

        });

    }
}
