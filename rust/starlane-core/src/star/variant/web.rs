use std::str::FromStr;

use std::thread;


use url::Url;

use crate::resource::{ResourceAddress};
use crate::star::{StarSkel};
use crate::starlane::api::{StarlaneApi, StarlaneApiRelay};
use tokio::sync::{oneshot, mpsc};
use crate::star::variant::{VariantCall, FrameVerdict};
use crate::util::{AsyncRunner, AsyncProcessor};
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime::Runtime;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::error::Error;
use bytes::BytesMut;
use httparse::{Request, Header};
use starlane_resources::http::{HttpRequest, Headers, HttpMethod, HttpResponse};
use starlane_resources::data::{BinSrc, DataSet};
use std::sync::Arc;
use starlane_resources::message::{ProtoMessage, ResourcePortMessage,MessageFrom};
use std::convert::TryInto;
use crate::frame::Reply;
use starlane_resources::{ResourcePath, ResourceStub};
use crate::parse::parse_host;
use handlebars::Handlebars;
use serde_json::json;
use starlane_resources::property::{ResourcePropertyValueSelector, ResourceRegistryPropertyValueSelector, ResourceValue, ResourceValues};
use std::future::Future;
use nom::AsBytes;


pub struct WebVariant {
    skel: StarSkel,
}

impl WebVariant {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<VariantCall>) {
        AsyncRunner::new(
            Box::new(Self { skel: skel.clone() }),
            skel.variant_api.tx.clone(),
            rx,
        );
    }
}

#[async_trait]
impl AsyncProcessor<VariantCall> for WebVariant {
    async fn process(&mut self, call: VariantCall) {
        match call {
            VariantCall::Init(tx) => {
                self.init(tx);
            }
            VariantCall::Frame { frame, session:_, tx } => {
                tx.send(FrameVerdict::Handle(frame));
            }
        }
    }
}


impl WebVariant {
    fn init(&self, tx: tokio::sync::oneshot::Sender<Result<(), crate::error::Error>>) {
        let api = StarlaneApi::new(self.skel.surface_api.clone());

        start(api);

        tx.send(Ok(())).unwrap_or_default();
    }
}

fn start(api: StarlaneApi) {
    thread::spawn(move || {

        let runtime = Runtime::new().unwrap();
        runtime.block_on( async move {

            match std::net::TcpListener::bind("127.0.0.1:8080") {
                Ok(std_listener) => {
                    let listener = TcpListener::from_std(std_listener).unwrap();
                    while let Ok((mut stream, _)) = listener.accept().await {
                        let api = api.clone();
                        tokio::spawn( async move {
                            match process_request(stream, api.clone()).await {
                                Ok(_) => {
                                    info!("ok");
                                }
                                Err(error) => {
                                    error!("{}",error);
                                }
                            }
                        });
                    }
                }
                Err(error) => {
                    error!("FATAL: could not setup TcpListener {}", error);
                }
            }
        });
    });
}

async fn process_request( mut stream: TcpStream, api: StarlaneApi ) -> Result<(),Error>{
    info!("received HTTP Stream...");

    let mut request_buf: Vec<u8> = vec![];
    let mut buf = [0 as u8; 16384]; // 16k read buffer

    let request = loop {
        match stream.read(&mut buf).await {
            Ok(size) => request_buf.extend(&buf[0..size]),
            Err(_) => {} // handle err,
        }
println!("ok...");
        let mut headers = [httparse::EMPTY_HEADER; 16];
        let mut req = Request::new(&mut headers);
        if let Ok(status) = req.parse(&request_buf) {

            if status.is_complete() {

                let mut http_headers = Headers::new();
                for header in req.headers {
                    http_headers.insert(header.name.to_string(), String::from_utf8(header.value.to_vec())?);
                }

                let method = HttpMethod::from_str(req.method.expect("expected method"))?;

                let body_offset = status.unwrap();
                let mut body:Vec<u8> = vec![];
                for index in body_offset..request_buf.len() {
                    body.push( request_buf.get(index).unwrap().clone() );
                }
                let body = BinSrc::Memory( Arc::new(body) );

                break HttpRequest {
                    path: req.path.expect("expected path").to_string(),
                    method: method,
                    headers: http_headers,
                    body
                };
            } else {
                println!("incomplete parse... ");
            }
        }
    };

    match create_response(request,api).await {
        Ok(response) => {
            stream.write(format!("HTTP/1.1 {} OK\r\n\r\n",response.status).as_bytes() ).await?;

            match response.body.unwrap() {
                BinSrc::Memory(body) => {
                    stream.write( body.as_bytes() ).await?;
                }
            }
        }
        Err(e) => {
eprintln!("ERROR: {}", e.to_string() );
            error_response(stream, 500, "Internal Server Error").await;
        }
    }

    Ok(())
}

async fn error_response( mut stream: TcpStream, code: usize, message: &str)  {
    stream.write(format!("HTTP/1.1 {} OK\r\n\r\n",code).as_bytes() ).await.unwrap();
    let messages = json!({"title": code, "message":message});
    stream.write(TEMPLATES.render("page", &messages ).unwrap().as_bytes() ).await.unwrap();
}

async fn create_response( request: HttpRequest, api: StarlaneApi ) -> Result<HttpResponse,Error> {

    let (_,host) = parse_host(request.headers.get("Host").ok_or("Missing HOST")?.as_str())?;

    // first thing we do is try to get a configuration for localhost
    let selector = ResourcePropertyValueSelector::Registry( ResourceRegistryPropertyValueSelector::Config );
    let path = ResourcePath::from_str(host)?;
println!("SENDING FOR VALUES...");
    let values = api.select_values( path, selector.clone() ).await;

    let values = match values {
        Ok(values) => {values}
        Err(fail) => {
            let mut response = HttpResponse::new();
            response.status = 404;
            let error = format!("It looks like you have just installed Starlane and haven't created a space for the '{}' domain yet.", host);
            let messages = json!({"title": "WELCOME", "message": error});
            response.body = Option::Some(BinSrc::Memory(Arc::new(TEMPLATES.render("page", &messages )?.as_bytes().to_vec())));
            return Ok(response);
        }
    };
println!("RECEIVED VALUES...");
    match values.values.get(&selector) {
        None => {
            let mut response = HttpResponse::new();
            response.status = 404;
            let error = format!("proxy configuration not found for host: '{}'", host);
            let messages = json!({"title": response.status, "message": error});
            response.body = Option::Some(BinSrc::Memory(Arc::new(TEMPLATES.render("page", &messages )?.as_bytes().to_vec())));
            Ok(response)
        }
        Some(value) => {
            if let ResourceValue::Config(config) = value {
                let mut response = HttpResponse::new();
                response.status = 200;
                let error = format!("found config '{}' for '{}' domain.", config.to_string(), host);
                let messages = json!({"title": response.status, "message": error});
                response.body = Option::Some(BinSrc::Memory(Arc::new(TEMPLATES.render("page", &messages )?.as_bytes().to_vec())));
                Ok(response)
            } else {
                let mut response = HttpResponse::new();
                response.status = 500;
                let error = "unexpected response when getting configSrc";
                let messages = json!({"title": response.status, "message": error});
                response.body = Option::Some(BinSrc::Memory(Arc::new(TEMPLATES.render("page", &messages )?.as_bytes().to_vec())));
                Ok(response)
            }
        }
    }



}


lazy_static! {
  pub static ref TEMPLATES: Handlebars<'static> = {
        let mut reg = Handlebars::new();
        reg.register_template_string("page", r#"

<!DOCTYPE html>
<html lang="en-US" style="background: black">

<head>
<meta charset="utf-8">
<title>STARLANE</title>

<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=Josefin+Sans:ital,wght@1,300&family=Jura&family=Stick+No+Bills:wght@200&display=swap" rel="stylesheet">
<link href="//cdn-images.mailchimp.com/embedcode/horizontal-slim-10_7.css" rel="stylesheet" type="text/css">

<style>



section{
  position: fixed;
  text-align: center;
  font-family: "jura", sans-serif;
  font-family: "Stick No Bills", sans-serif;
  font-family: "Josefin Sans", sans-serif;

  left: 50%;
  top: 50%;
  transform: translate(-50%,-50%);


}
#title{
  display: block;
  font-weight: 300;
  font-size: 196px;
  text-align: center;

  font-family: "Josefin Sans", sans-serif;
  background: -webkit-linear-gradient(white, #38495a);
  background: -webkit-linear-gradient(white, #eeaa5a);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  letter-spacing: 5px;
}

#message{
  font-weight: 200;
  font-size: 32px;

  font-family: "Josefin Sans", sans-serif;
  background: -webkit-linear-gradient(white, #38495a);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  letter-spacing: 2px;
}


</style>


</head>
<body>

<section>
<span id="title">{{ title }}</span>
<span id="message">{{ message }}</span>
</section>



</body>
</html>






  "#);
        reg
    };

}
