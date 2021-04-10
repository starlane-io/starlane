use tokio::sync::mpsc;
use tokio::sync::oneshot;
use crate::provision::Provisioner;
use crate::error::Error;
use crate::template::{ConstellationTemplate, StarKeyTemplate, StarKeyConstellationTemplate, StarKeyIndexTemplate};
use crate::layout::ConstellationLayout;
use crate::proto::{ProtoStar, local_lanes, ProtoLane};
use crate::star::{StarKey, Star};
use std::collections::{HashSet, HashMap};
use std::sync::mpsc::{Sender, Receiver};
use crate::message::LaneGram;
use std::sync::Arc;

pub struct Starlane
{
    pub tx: mpsc::Sender<StarlaneCommand>,
    rx: mpsc::Receiver<StarlaneCommand>,
    stars: HashMap<StarKey,StarConnector>
}

impl Starlane
{
    pub fn new()->Self
    {
        let (tx, rx) = mpsc::channel(32);
        Starlane{
            stars: HashMap::new(),
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

    async fn provision( &mut self, template: ConstellationTemplate )->Result<(),Error>
    {
        let mut proto_constellation = vec![];
        for star_template in template.stars
        {
            let key = self.create_star_key(&star_template.key);
            let proto_star = Arc::new(ProtoStar::new(key.clone(), star_template.kind.clone() ));
            proto_constellation.push(proto_star.clone());
            //self.stars.insert(key.clone(), StarConnector::ProtoStar(proto_star.clone()) );
            println!("creating proto star: {:?} key: {}", &star_template.kind, key );
        }

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
        let index = match &template.index{
            StarKeyIndexTemplate::Central => {0 as _}
            StarKeyIndexTemplate::Exact(index) => {index.clone()}
        };
        StarKey::new_with_constellation(constellation, index)
    }

}

pub enum StarConnector
{
    Star(Star),
    ProtoStar(ProtoStar)
}

impl StarConnector
{
    pub fn connect( &mut self )->ProtoLane
    {
        let (big,small) = local_lanes();
        match self{
            StarConnector::Star( star ) => {
               unimplemented!()
            }
            StarConnector::ProtoStar( proto) => {
                proto.add_lane(small)
            }
        }

        big
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
            tx.send(StarlaneCommand::Destroy ).await;

            handle.await;

        });

    }
}
