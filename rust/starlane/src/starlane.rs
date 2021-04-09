use tokio::sync::mpsc;
use tokio::sync::oneshot;
use crate::provision::Provisioner;
use crate::error::Error;
use crate::template::ConstellationTemplate;
use crate::layout::ConstellationLayout;

pub struct Starlane
{
    pub tx: mpsc::Sender<StarlaneCommand>,
    rx: mpsc::Receiver<StarlaneCommand>
}

impl Starlane
{
    pub fn new()->Self
    {
        let (tx, rx) = mpsc::channel(32);
        Starlane{
            tx:tx,
            rx: rx
        }
    }

    pub async fn run(&mut self)
    {
        while let Option::Some(command) = self.rx.recv().await
        {
            match command
            {
                StarlaneCommand::Provision(command) => {
                    command.oneshot.send(Ok(()));
                }
                StarlaneCommand::Hello => {
                    println!("Goodbye");
                }
                StarlaneCommand::Destroy => {
                    println!("closing rx");
                    self.rx.close();
                }

            }
        }
    }

}

pub enum StarlaneCommand
{
    Hello,
    Provision(ProvisionConstellationCommand),
    Destroy
}

pub struct ProvisionConstellationCommand
{
    template: ConstellationTemplate,
    oneshot: oneshot::Sender<Result<(),Error>>
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


#[cfg(test)]
mod test
{
    use tokio::runtime::Runtime;
    use crate::starlane::{Starlane, StarlaneCommand};

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

            tx.send(StarlaneCommand::Hello ).await;
            tx.send(StarlaneCommand::Destroy ).await;

            handle.await;

        });



    }
}
