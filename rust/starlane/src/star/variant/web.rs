use crate::star::{StarSkel};
use crate::star::variant::{StarVariant, StarVariantCommand};
use crate::error::Error;
use std::thread;
use tokio::runtime::Runtime;
use actix_http::{HttpService, Response, Request};
use actix_http::http::HeaderValue;
use actix_server::Server;
use futures::future;
use tokio::sync::oneshot;

pub struct WebVariant
{
    skel: StarSkel,
}

impl WebVariant
{
    pub async fn new(skel: StarSkel) -> WebVariant
    {
        WebVariant
        {
            skel: skel.clone(),
        }
    }
}


#[async_trait]
impl StarVariant for WebVariant
{

    async fn init(&self, tx: oneshot::Sender<Result<(),Error>>) {

        start();

        tx.send(Ok(()));
    }
}

fn start(){
    thread::spawn(|| {
                      run();
                  });
}

#[actix_rt::main]
async fn run() -> Result<(), Error> {

    Server::build()
        .bind("starlane", "127.0.0.1:8080", || {
            HttpService::build()
                .client_timeout(1000)
                .client_disconnect(1000)
                .finish(|_req:Request| {
                    println!("{}", _req.path());
                    let mut res = Response::Ok();
                    res.header("x-head", HeaderValue::from_static("dummy value!"));
                    future::ok::<_, ()>(res.body("Hello world!"))
                })
                .tcp()
        })?
        .run()
        .await;

    Ok(())
}