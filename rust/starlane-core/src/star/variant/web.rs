use std::collections::HashMap;
use std::str::FromStr;

use std::thread;


use url::Url;

use crate::star::{StarSkel};
use tokio::sync::{oneshot, mpsc};
use crate::star::variant::{VariantCall, FrameVerdict};
use crate::util::{AsyncRunner, AsyncProcessor};
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime::Runtime;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::error::Error;
use bytes::BytesMut;
use httparse::{Request, Header};
use std::sync::Arc;
use std::convert::TryInto;
use handlebars::Handlebars;
use serde_json::json;
use std::future::Future;
use ascii::IntoAsciiString;
use http::{HeaderMap, HeaderValue, Response, Uri, Version};
use http::header::{HeaderName, HOST};
use mesh_portal::version::latest::bin::Bin;
use mesh_portal::version::latest::entity::request::{Method, RequestCore};
use mesh_portal::version::latest::id::{Point, Meta};
use mesh_portal::version::latest::messaging;
use nom::AsBytes;
use nom_supreme::error::ErrorTree;
use nom_supreme::final_parser::final_parser;
use crate::artifact::ArtifactRef;
use crate::cache::ArtifactItem;
use crate::html::HTML;
use regex::Regex;
use crate::particle::ArtifactSubKind;
use serde::{Serialize,Deserialize};
use tiny_http::{HeaderField, Server, StatusCode};
use mesh_portal_versions::version::v0_0_1::messaging::AsyncMessengerAgent;
use crate::message::StarlaneMessenger;
use crate::star::variant::web::parse::host_and_port;

lazy_static! {
//    pub static ref DATA_DIR: Mutex<String> = Mutex::new("data".to_string());
}
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
                self.init_web(tx);
            }
            VariantCall::Frame { frame, session:_, tx } => {
                tx.send(FrameVerdict::Handle(frame));
            }
        }
    }
}


impl WebVariant {
    fn init_web(&self, tx: tokio::sync::oneshot::Sender<Result<(), crate::error::Error>>) {
        let messenger = StarlaneMessenger::new(self.skel.surface_api.clone());

        start(messenger, self.skel.clone());

        tx.send(Ok(())).unwrap_or_default();
    }
}

fn start(messenger: StarlaneMessenger, skel: StarSkel) {
    thread::spawn(move || {

        let runtime = Runtime::new().unwrap();
        runtime.block_on( async move {

            let STARLANE_WEB_PORT = std::env::var("STARLANE_WEB_PORT").unwrap_or("8080".to_string());
            info!("STARLANE_WEB_PORT: {}",STARLANE_WEB_PORT);
            let server = Server::http(format!("0.0.0.0:{}",STARLANE_WEB_PORT)).unwrap();
            for req in server.incoming_requests() {

                info!("handling incoming http request");
                handle(req, messenger.clone(), skel.clone());
            }
        });
    });
}

fn handle( req: tiny_http::Request, api: StarlaneMessenger, skel: StarSkel ) {
println!("handling web connection...");
    tokio::spawn( async move {
        async fn process(mut req: tiny_http::Request, api: StarlaneMessenger, skel: StarSkel ) -> Result<(),Error> {
            let mut builder = http::Request::builder();
            builder = builder.uri(req.url()).method( req.method().to_string().as_str() );

            for header in req.headers() {
                builder = builder.header(header.field.to_string().as_str(), header.value.as_str() );
            }
            let request = match req.body_length().as_ref() {
                None => {
                    builder.body(Arc::new(vec![]))?
                }
                Some(len) => {
                    let mut buf = Vec::with_capacity(*len);
                    let reader = req.as_reader();
                    reader.read_to_end(&mut buf);
                    let buf = Arc::new(buf);

                    builder.body(buf)?
                }
            };

            let response = process_request(request, api.clone(), skel.clone() ).await?;
            let mut headers = vec![];
            for (name,value) in response.headers() {
                let header = tiny_http::Header{
                    field: HeaderField::from_str(name.as_str() )?,
                    value: value.to_str()?.into_ascii_string()?
                };
                headers.push(header);
            }
            let data_length = Some(response.body().len());
            let response = tiny_http::Response::new( tiny_http::StatusCode(response.status().as_u16()), headers, response.body().as_slice(), data_length, None  );
            req.respond(response);
            Ok(())
        }

        match process(req, api.clone(), skel.clone() ).await {
            Ok(_) => {
            }
            Err(err) => {
                error!("{}",err.to_string());
//                        error_response(req, 500, "Server Error");
            }
        }
    });
}


async fn error_response( mut req: tiny_http::Request, status: u16, message: &str)  {
    let messages = json!({"title": status.to_string(), "message":message});
    let html = HTML.render("error-code-page", &messages ).unwrap();
    let mut response =  tiny_http::Response::from_string(html);
    let mut response = response.with_status_code(StatusCode(status));
    match req.respond(response) {
        Ok(_) => {}
        Err(err) => {error!("{}",err.to_string())}
    }
}

async fn process_request( http_request: http::Request<Bin>, api: StarlaneMessenger, skel: StarSkel ) -> Result<http::Response<Bin>,Error> {

    let host_and_port = http_request.headers().get("Host").ok_or("HOST header not set")?;
    let host = host_and_port.to_str()?.split(":").next().ok_or("expected host")?.to_string();
    let core = RequestCore::from(http_request);
    let to = Point::from_str( host.as_str() )?;
    let from = skel.info.point;
    let request = messaging::Request::new( core, from, to );
println!("exchanging...to :{}", request.to.to_string() );
    let response = skel.messaging_api.request(request).await;
println!("got response...(status: {})",response.core.status.as_u16());
    if !response.core.status.is_success() {
        let error = response.core.status.canonical_reason().unwrap_or("Unknown");
        let messages = json!({"title": response.core.status.as_u16().to_string(), "message": error});
        let body  = HTML.render("error-code-page", &messages )?;
        let mut builder: http::response::Builder = response.core.try_into()?;
        return Ok(builder.body(Arc::new(body.as_bytes().to_vec()))?)
    }

    let response = response.core.try_into()?;

    Ok(response)
}



mod tests {

}
#[cfg(test)]
mod test {
    use mesh_portal_versions::version::v0_0_1::span::new_span;
    use crate::error::Error;
    use regex::Regex;
    use crate::star::variant::web::HostAndPort;
    use crate::star::variant::web::parse::host_and_port;

    #[test]
    pub async fn path_regex() -> Result<(),Error> {
        let regex = Regex::new("/files/")?;
        assert!(regex.is_match("/files/"));


        let regex = Regex::new("/files/.*")?;
        assert!(regex.is_match("/files/"));

        let regex = Regex::new("/files/(.*)")?;
        assert!(regex.is_match("/files/some-path"));
        assert_eq!("/some-path".to_string(),regex.replace("/files/some-path", "/$1").to_string());


        let regex = Regex::new("/files/(.*)")?;
        assert!(regex.is_match("/files/some/path.html"));
        assert_eq!("/some/path.html".to_string(),regex.replace("/files/some/path.html", "/$1").to_string());
        Ok(())
    }

    #[test]
    pub async fn host() -> Result<(),Error> {
        let host_and_port:HostAndPort = host_and_port(new_span("localhost:8080"))?;
        assert_eq!( host_and_port.host, "localhost".to_string() );
        assert_eq!( host_and_port.port, 8080 );
        Ok(())
    }
}

pub struct HostAndPort {
    pub host: String,
    pub port: u32
}

pub mod parse {
    use std::num::ParseIntError;
    use std::str::FromStr;
    use mesh_portal_versions::error::MsgErr;
    use mesh_portal_versions::version::v0_0_1::parse::error::result;
    use nom::bytes::complete::{is_a, tag, take_while};
    use nom::character::is_digit;
    use nom::error::{ErrorKind, ParseError, VerboseError};
    use nom::sequence::tuple;
    use nom_supreme::error::ErrorTree;
    use cosmic_nom::Span;
    use mesh_portal_versions::version::v0_0_1::parse::domain;
    use crate::star::variant::web::HostAndPort;

    pub fn host_and_port<I:Span>(input: I ) -> Result<HostAndPort,MsgErr> {
        let input = input;
        let (host,_,port) = result(tuple(( domain, tag(":"), is_a("0123456789")  ) )(input.clone()))?;

        let host = host.to_string();
        let port= port.to_string();
        let port = match u32::from_str(port.as_str() ) {
            Ok(port) => port,
            Err(err) => {
                return Err(MsgErr::from_500(format!("bad port {}", port ).as_str() ));
            }
        };
        let host_and_port = HostAndPort {
            host,
            port
        };
        Ok(host_and_port)
    }

}
