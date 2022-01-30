use std::str::FromStr;

use std::thread;


use url::Url;

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
use std::sync::Arc;
use std::convert::TryInto;
use handlebars::Handlebars;
use serde_json::json;
use std::future::Future;
use mesh_portal_serde::version::latest::entity::request::Http;
use mesh_portal_serde::version::latest::id::Meta;
use mesh_portal_serde::version::latest::payload::Payload;
use nom::AsBytes;
use crate::artifact::ArtifactRef;
use crate::cache::ArtifactItem;
use crate::html::HTML;
use regex::Regex;
use crate::resource::ArtifactKind;
use crate::resources::message::ProtoRequest;
use serde::{Serialize,Deserialize};


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
        let api = StarlaneApi::new(self.skel.surface_api.clone());

        start(api,self.skel.clone());

        tx.send(Ok(())).unwrap_or_default();
    }
}

fn start(api: StarlaneApi,skel: StarSkel) {
    thread::spawn(move || {

        let runtime = Runtime::new().unwrap();
        runtime.block_on( async move {

            match std::net::TcpListener::bind("127.0.0.1:8080") {
                Ok(std_listener) => {
                    let listener = TcpListener::from_std(std_listener).unwrap();
                    while let Ok((mut stream, _)) = listener.accept().await {
                        let api = api.clone();
                        let skel = skel.clone();
                        tokio::spawn( async move {
                            match process_request(stream, api.clone(),skel).await {
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

async fn process_request( mut stream: TcpStream, api: StarlaneApi, skel: StarSkel ) -> Result<(),Error>{
    unimplemented!()
    /*
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

                break Http{
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

    match create_response(request,api,skel).await {
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

     */
}

async fn error_response( mut stream: TcpStream, code: usize, message: &str)  {
    stream.write(format!("HTTP/1.1 {} OK\r\n\r\n",code).as_bytes() ).await.unwrap();
    let messages = json!({"title": code, "message":message});
    stream.write(HTML.render("error-code-page", &messages ).unwrap().as_bytes() ).await.unwrap();
}

async fn create_response( request: Http, api: StarlaneApi, skel: StarSkel ) -> Result<HttpResponse,Error> {
    /*

    let (_,shell) = parse_host(request.headers.get("Host").ok_or("Missing HOST")?.as_str())?;

    // first thing we do is try to get a configuration for localhost
    let selector = ResourcePropertyValueSelector::Registry( ResourceRegistryPropertyValueSelector::Config );
    let path = ResourcePath::from_str(shell)?;
println!("SENDING FOR VALUES...");
    let values = api.select_values( path, selector.clone() ).await;

    let values = match values {
        Ok(values) => {values}
        Err(fail) => {
            let mut response = HttpResponse::new();
            response.status = 404;
            let error = format!("It looks like you have just installed Starlane and haven't created a space for the '{}' shell yet.", shell);
            let messages = json!({"title": "WELCOME", "message": error});
            response.body = Option::Some(BinSrc::Memory(Arc::new(HTML.render("error-code-page", &messages )?.as_bytes().to_vec())));
            return Ok(response);
        }
    };
println!("RECEIVED VALUES...");
    match values.values.get(&selector) {
        None => {
            let mut response = HttpResponse::new();
            response.status = 404;
            let error = format!("proxy configuration not found for shell: '{}'", shell);
            let messages = json!({"title": response.status, "message": error});
            response.body = Option::Some(BinSrc::Memory(Arc::new(HTML.render("error-code-page", &messages )?.as_bytes().to_vec())));
            Ok(response)
        }
        Some(value) => {
            if let ResourceValue::Config(config) = value {
                match config {
                    ConfigSrc::None => {
                        let mut response = HttpResponse::new();
                        response.status = 404;
                        let error = format!("The '{}' Space is there, but it doesn't have a router config assigned yet.", shell);
                        let messages = json!({"title": shell, "message": error});
                        response.body = Option::Some(BinSrc::Memory(Arc::new(HTML.render("error-code-page", &messages )?.as_bytes().to_vec())));
                        Ok(response)
                    }
                    ConfigSrc::Artifact(artifact) => {


                        let factory = skel.machine.get_proto_artifact_caches_factory().await?;
                        let mut caches = factory.create();

                        let artifact_ref = ArtifactRef {
                            address: artifact.clone(),
                            kind: ArtifactKind::HttpRouter
                        };

                        if let Result::Err(err) = caches.cache(vec![artifact_ref] ).await {
eprintln!("Error: {}",err.to_string());

                            let mut response = HttpResponse::new();
                            response.status = 404;
                            let error = format!("could not cache router config: '{}' Are you sure it's there?", artifact.to_string() );
                            let messages = json!({"title": "404", "message": error});
                            response.body = Option::Some(BinSrc::Memory(Arc::new(HTML.render("error-code-page", &messages )?.as_bytes().to_vec())));

                            return Ok(response)
                        }
                        let caches = caches.to_caches().await?;
                        let config = match caches.http_router_config.get(artifact) {
                            None => {
                                let mut response = HttpResponse::new();
                                response.status = 404;
                                let error = format!("cannot locate router config: '{}'", artifact.to_string() );
                                let messages = json!({"title": "404", "message": error});
                                response.body = Option::Some(BinSrc::Memory(Arc::new(HTML.render("error-code-page", &messages )?.as_bytes().to_vec())));
                                return Ok(response)
                            }
                            Some(config) => {
                                config
                            }
                        };


                        for mapping in &config.mappings {


                            if mapping.path_pattern.is_match(request.path.as_str() ) {


                                let resource = mapping.path_pattern.replace( request.path.as_str(), mapping.resource_pattern.as_str()  ).to_string();
                                let resource = ResourcePath::from_str(resource.as_str() )?;

                                let mut proto = ProtoMessage::new();
                                proto.payload( request.clone() );
                                proto.to( resource.into() ) ;
                                proto.from(MessageFrom::Inject);
                                match api.send_http_message(proto, ReplyKind::HttpResponse, "sending an Http").await {
                                    Ok(reply) => {
                                        if let Reply::HttpResponse(response ) = reply {
                                            return Ok(response)
                                        } else {
                                           return Err("unexpected reply".into() );
                                        }
                                    }
                                    Err(error) => {
                                        let mut response = HttpResponse::new();
                                        response.status = 404;
                                        let error = "NOT FOUND".to_string();
                                        let messages = json!({"title": "404", "message": error});
                                        response.body = Option::Some(BinSrc::Memory(Arc::new(HTML.render("error-code-page", &messages)?.as_bytes().to_vec())));
                                        return Ok(response);
                                    }
                                }
                            }
                        }


                        let mut response = HttpResponse::new();
                        response.status = 200;
                        let error = format!("Host: '{}' is using router config: '{}'", shell, artifact.to_string());
                        let messages = json!({"title": "CONFIGURED", "message": error});
                        response.body = Option::Some(BinSrc::Memory(Arc::new(HTML.render("error-code-page", &messages)?.as_bytes().to_vec())));
                        Ok(response)
                    }
                }

            } else {
                let mut response = HttpResponse::new();
                response.status = 500;
                let error = format!("received an unexpected value when trying to get router config");
                let messages = json!({"title": "500", "message": error});
                response.body = Option::Some(BinSrc::Memory(Arc::new(HTML.render("error-code-page", &messages )?.as_bytes().to_vec())));
                Ok(response)
            }
        }
    }




     */
    unimplemented!()
}
#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct HttpResponse{
    pub status: usize,
    pub headers: Meta,
    pub body: Payload
}

impl HttpResponse {
    pub fn new( ) -> HttpResponse {
        Self {
            status: 200,
            headers: Meta::new(),
            body: Payload::Empty
        }
    }
}



mod tests {

}
#[cfg(test)]
mod test {
    use crate::error::Error;
    use regex::Regex;

    #[test]
    pub fn path_regex() -> Result<(),Error> {
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
}