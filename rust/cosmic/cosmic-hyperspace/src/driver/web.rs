use crate::driver::{
    Driver, DriverCtx, DriverSkel, HyperDriverFactory, HyperItemSkel, HyperSkel, ItemHandler,
    ItemSkel, ItemSphere,
};
use crate::star::HyperStarSkel;
use crate::Cosmos;
use cosmic_space::artifact::ArtRef;
use cosmic_space::config::bind::BindConfig;
use cosmic_space::fail::http;
use cosmic_space::kind::{BaseKind, Kind, NativeSub};
use cosmic_space::loc::Point;
use cosmic_space::parse::bind_config;
use cosmic_space::selector::KindSelector;
use cosmic_space::util::log;
use cosmic_space::wave::core::http2::{HttpMethod, HttpRequest};
use cosmic_space::wave::core::{DirectedCore, HeaderMap, ReflectedCore};
use std::str::FromStr;
use std::sync::Arc;
use std::thread;
use tiny_http::Server;
use tokio::runtime::Runtime;
use url::Url;
use cosmic_space::substance::{Bin, Substance};
use cosmic_space::wave::{DirectedProto, Ping, Wave};
use cosmic_space::wave::exchange::asynch::{InCtx, ProtoTransmitter};
use ascii::IntoAsciiString;
use crate::err::HyperErr;

lazy_static! {
    static ref WEB_BIND_CONFIG: ArtRef<BindConfig> = ArtRef::new(
        Arc::new(web_bind()),
        Point::from_str("GLOBAL::repo:1.0.0:/bind/web.bind").unwrap()
    );
}

fn web_bind() -> BindConfig {
    log(bind_config(
        r#"
    Bind(version=1.0.0)
    {
       Route {
         Http<*> -> (()) => &;
       }
    }
    "#,
    ))
    .unwrap()
}

pub struct WebDriverFactory;

impl WebDriverFactory {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl<P> HyperDriverFactory<P> for WebDriverFactory
where
    P: Cosmos,
{
    fn kind(&self) -> KindSelector {
        KindSelector::from_str("Native<Web>").unwrap()
    }

    async fn create(
        &self,
        _: HyperStarSkel<P>,
        driver_skel: DriverSkel<P>,
        _: DriverCtx,
    ) -> Result<Box<dyn Driver<P>>, P::Err> {

        Ok(Box::new(WebDriver::new(driver_skel)))
    }
}

pub struct WebDriver<P> where P: Cosmos
{
    skel: DriverSkel<P>
}

impl <P> WebDriver<P> where P: Cosmos {
    pub fn new(skel: DriverSkel<P>) -> Self {
        Self {
            skel
        }
    }
}

#[async_trait]
impl<P> Driver<P> for WebDriver<P>
where
    P: Cosmos,
{
    fn kind(&self) -> Kind {
        Kind::Native(NativeSub::Web)
    }

    async fn item(&self, point: &Point) -> Result<ItemSphere<P>, P::Err> {
        let skel = ItemSkel::new( point.clone(), Kind::Native(NativeSub::Web), self.skel.clone());
        Ok(ItemSphere::Handler(Box::new(Web::new(skel) )))
    }
}

pub struct Web<P> where P: Cosmos{
    skel: ItemSkel<P>
}


#[handler]
impl <P> Web<P> where P: Cosmos {
    pub fn new(skel: ItemSkel<P>) -> Self {
       Self { skel }
    }

    #[route("Http<*>")]
    pub async fn handle( &self, _: InCtx<'_,Bin>) -> Result<ReflectedCore,P::Err> {
        Ok(ReflectedCore::ok())
    }
}

#[async_trait]
impl<P> ItemHandler<P> for Web<P>
where
    P: Cosmos,
{
    async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err> {
        Ok(WEB_BIND_CONFIG.clone())
    }
}

pub struct WebRunner<P>
where
    P: Cosmos,
{
    pub skel: ItemSkel<P>,
    pub transmitter: ProtoTransmitter
}

impl<P> WebRunner<P>
where
    P: Cosmos,
{
    pub fn start(mut self) {
        thread::spawn(move || {
            let runtime = Runtime::new().unwrap();
            runtime.block_on(async move {
                let STARLANE_WEB_PORT =
                    std::env::var("STARLANE_WEB_PORT").unwrap_or("8080".to_string());
                let server = Server::http(format!("0.0.0.0:{}", STARLANE_WEB_PORT)).unwrap();
                for req in server.incoming_requests() {
                    self.handle(req);
                }
            });
        });
    }

    async fn handle(&self, mut req: tiny_http::Request) -> Result<(), P::Err> {
        let method = HttpMethod::from_str(req.method().to_string().as_str())?;
        let mut headers = HeaderMap::new();
        for header in req.headers() {
            headers.insert(header.field.to_string(), header.value.to_string());
        }
        let uri: Url = Url::from_str(req.url())?;
        let body = Substance::Bin(match req.body_length().as_ref() {
            None => Arc::new(vec![]),
            Some(len) => {
                let mut buf: Vec<u8> = Vec::with_capacity(*len);
                let reader = req.as_reader();
                reader.read_to_end(&mut buf);
                let buf = Arc::new(buf);
                buf
            }
        });

        let request = HttpRequest {
            method,
            headers,
            uri,
            body
        };

        let core: DirectedCore = request.into();

        let mut wave = DirectedProto::ping();
        wave.core(core);
        let pong = self.transmitter.ping(wave).await?;

        let body = pong.core.body.clone().to_bin()?;
           let mut headers = vec![];
           for (name, value) in pong.core.headers.clone() {
               let header = tiny_http::Header {
                   field: tiny_http::HeaderField::from_str(name.as_str())?,
                   value: value.into_ascii_string()?
               };
               headers.push(header);
           }
           let data_length = Some(body.len());
           let response = tiny_http::Response::new(
               tiny_http::StatusCode(pong.core.status.as_u16()),
               headers,
               body.as_slice(),
               data_length,
               None,
           );
           req.respond(response);
        Ok(())
    }
}
