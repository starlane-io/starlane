use tokio::sync::mpsc;
use tokio::sync::oneshot;
use crate::provision::Provisioner;
use crate::error::Error;
use crate::template::{ConstellationTemplate, StarKeyTemplate, StarKeyConstellationTemplate, StarKeyIndexTemplate};
use crate::layout::ConstellationLayout;
use crate::proto::{ProtoStar, local_tunnels, ProtoTunnel, ProtoStarController};
use crate::star::{StarKey, Star, StarController, StarCommand};
use std::collections::{HashSet, HashMap};
use std::sync::mpsc::{Sender, Receiver};
use crate::frame::Frame;
use std::sync::Arc;
use crate::lane::{Lane, LocalTunnelConnector };
use std::cmp::Ordering;

pub struct Starlane
{
    pub tx: mpsc::Sender<StarlaneCommand>,
    rx: mpsc::Receiver<StarlaneCommand>,
    star_controllers: HashMap<StarKey,StarController>
}

impl Starlane
{
    pub fn new()->Self
    {
        let (tx, rx) = mpsc::channel(32);
        Starlane{
            star_controllers: HashMap::new(),
            tx: tx,
            rx: rx
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
                    let result = self.provision(command.template).await;
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

    async fn provision( &mut self, template: ConstellationTemplate )->Result<(),Error>
    {
        let mut map = HashMap::new();
        for star_template in &template.stars
        {
            let key = self.create_star_key(&star_template.key);
            map.insert( star_template.key.clone(), key.clone() );
            let (mut proto_star,proto_star_ctrl) = ProtoStar::new(key.clone(), star_template.kind.clone() );
            self.star_controllers.insert(key.clone(), proto_star_ctrl );
            tokio::spawn( async move { proto_star.evolve().await; } );

            println!("creating proto star: {:?} key: {}", &star_template.kind, key );
        }

        // now make the LANES
        for star_template in &template.stars
        {
            for lane in &star_template.lanes
            {
                let local = map.get(&star_template.key );
                let second = map.get(&lane.star );

                if local.is_none()
                {
                    return Err(format!("could not find local star_key {:?}",&star_template.key).into());
                }

                if second.is_none()
                {
                    return Err(format!("could not find secondstar_key {:?}",&star_template.key).into());
                }

                let local = local.unwrap().clone();
                let second = second.unwrap().clone();


                self.add_local_lane(local, second ).await;
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

        let high_lane= Lane::new(low).await;
        let low_lane = Lane::new(high).await;
        let connector = LocalTunnelConnector::new(&high_lane,&low_lane).await?;
        high_star_ctrl.command_tx.send(StarCommand::AddLane(high_lane)).await;
        low_star_ctrl.command_tx.send(StarCommand::AddLane(low_lane)).await;
        high_star_ctrl.command_tx.send( StarCommand::AddConnectorController(connector)).await;

        Ok(())
    }

    fn create_star_key( &mut self, template: &StarKeyTemplate )->StarKey
    {
        let constellation = match &template.constellation{
            StarKeyConstellationTemplate::Central => {
                vec![]
            }
            StarKeyConstellationTemplate::Path(path) => {
                path.clone()
            }
        };
        let index = match &template.index {
            StarKeyIndexTemplate::Central => {0 as _}
            StarKeyIndexTemplate::Exact(index) => {index.clone()}
        };
        StarKey::new_with_constellation(constellation, index)
    }

}

pub enum StarlaneCommand
{
    Connect(ConnectCommand),
    ProvisionConstellation(ProvisionConstellationCommand),
    Destroy
}

pub struct ProvisionConstellationCommand
{
    template: ConstellationTemplate,
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
    pub fn new(template: ConstellationTemplate)->(Self,oneshot::Receiver<Result<(),Error>>)
    {
        let (tx,rx)= oneshot::channel();
        (ProvisionConstellationCommand{
            template: template,
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
    use crate::starlane::{Starlane, StarlaneCommand, ProvisionConstellationCommand};
    use crate::template::ConstellationTemplate;
    use crate::error::Error;
    use tokio::sync::oneshot::error::RecvError;
    use tokio::time::Duration;

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
                let (command, mut rx) = ProvisionConstellationCommand::new(ConstellationTemplate::new_standalone());
                tx.send(StarlaneCommand::ProvisionConstellation(command)).await;
                let result = rx.await;
                match result{
                    Ok(result) => {
                        match result{
                            Ok(_) => {println!("template ok.")}
                            Err(e) => {println!("{}", e)}
                        }
                    }
                    Err(e) => {println!("{}", e)}
                }
            }
            tokio::time::sleep(Duration::from_secs(10)).await;

            tx.send(StarlaneCommand::Destroy ).await;

            handle.await;

        });

    }
}
