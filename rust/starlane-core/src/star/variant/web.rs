use std::str::FromStr;

use std::thread;


use url::Url;

use crate::resource::ResourceAddress;
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
use starlane_resources::ResourcePath;


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
info!("LISTENING to 8080");
                    let listener = TcpListener::from_std(std_listener).unwrap();
                    while let Ok((mut stream, _)) = listener.accept().await {
                        match process_request(stream, api.clone()).await {
                            Ok(_) => {
                                info!("ok");
                            }
                            Err(error) => {
                                error!("{}",error);
                            }
                        }
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
        let mut headers = [httparse::EMPTY_HEADER; 16];
        let mut req = Request::new(&mut headers);
        if let Ok(status) = req.parse(&request_buf) {

            if status.is_complete() {
                info!("path is {}", req.path.expect("expected path "));

                let mut http_headers = Headers::new();
                for header in req.headers {
                    http_headers.insert(header.name.to_string(), String::from_utf8(header.value.to_vec())?);
                }

                let method = HttpMethod::from_str(req.method.expect("expected method"))?;

                let body_offset = status.unwrap();
info!("body offset: {}", body_offset);
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
            }
        }
    };

    info!("PATH: {}",request.path);
    info!("method: {}",request.method.to_string() );
    info!("headers: {}",request.headers.len());

    for (k,v) in &request.headers {
        info!("... header {}={}", k,v );
    }

    let mut payload = DataSet::new();
    payload.insert("request".to_string(), request.try_into()? );


    let mut proto = ProtoMessage::new();
    proto.to = Option::Some(ResourcePath::from_str("hyperspace:starlane:appy:main")?.into());
    proto.from = Option::Some(MessageFrom::Inject);
    proto.payload = Option::Some(ResourcePortMessage {
        port: "web".to_string(),
        payload
    });

    proto.validate()?;
    let message = proto.create()?;

    info!("SENDING MESSAGE");

    let reply = api.send(message, "sending http request").await?;

    info!("Received Reply!");
    if let Reply::Port(payload) = reply {
        let response = payload.get("response").cloned().ok_or("expected 'response'")?;
        let response : HttpResponse = response.try_into()?;

        match response.body {
            BinSrc::Memory(bin) => {
                stream.write( bin.as_slice() ).await?;
                return Ok(());
            }
        }

    }

    stream.write(b"ERROR").await?;

    Ok(())
}



