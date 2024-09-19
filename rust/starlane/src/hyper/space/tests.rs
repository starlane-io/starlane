#![cfg(test)]

use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;

use crate::hyper::lane::HyperClient;
use crate::driver::control::ControlClient;
use crate::hyper::space::err::CosmicErr;
use crate::hyper::space::machine::MachineApiExtFactory;
use crate::hyper::space::mem::cosmos::MemCosmos;
use crate::hyper::space::mem::registry::MemRegCtx;
use crate::hyper::space::star::HyperStarApi;
use crate::hyper::space::platform::Platform;
use starlane_space::command::common::StateSrc;
use starlane_space::command::direct::create::{
    Create, PointSegTemplate, PointTemplate, Strategy, Template,
};
use starlane_space::command::{CmdTransfer, RawCommand};
use starlane_space::hyper::{
    Assign, AssignmentKind, HyperSubstance, ParticleLocation, ParticleRecord,
};
use starlane_space::kind::Kind;
use starlane_space::loc::{Layer, StarHandle, StarKey, ToSurface};
use starlane_space::log::{LogSource, RootLogger, StdOutAppender};
use starlane_space::particle::{Details, Properties, Status, Stub};
use starlane_space::point::Point;
use starlane_space::settings::Timeouts;
use starlane_space::substance::Substance;
use starlane_space::wave::core::cmd::CmdMethod;
use starlane_space::wave::core::ext::ExtMethod;
use starlane_space::wave::core::hyp::HypMethod;
use starlane_space::wave::core::{Method, ReflectedCore};
use starlane_space::wave::exchange::asynch::Exchanger;
use starlane_space::wave::{Agent, DirectedProto, Pong, Wave};
use starlane_space::HYPERUSER;

lazy_static! {
    pub static ref LESS: Point = Point::from_str("space:users:less").expect("point");
    pub static ref FAE: Point = Point::from_str("space:users:fae").expect("point");
}

#[async_trait]
pub trait Test: Sync + Send + Copy {
    async fn run(&self, client: ControlClient) -> Result<(), CosmicErr> {
        Ok(())
    }
}

pub fn harness<F>(mut f: F) -> Result<(), CosmicErr>
where
    F: Test,
{
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        let platform = MemCosmos::new();
        let machine_api = platform.machine();
        let logger = RootLogger::new(LogSource::Core, Arc::new(StdOutAppender()));
        let logger = logger.point(Point::from_str("test")?);

        tokio::time::timeout(Duration::from_secs(10), machine_api.wait_ready())
            .await
            .unwrap();

        let factory = MachineApiExtFactory {
            machine_api: machine_api.clone(),
            logger: logger.clone(),
        };

        let client = ControlClient::new(Box::new(factory))?;
        client.wait_for_ready(Duration::from_secs(5)).await?;

        f.run(client).await?;

        machine_api.terminate();
        Ok(())
    })
}

async fn create(
    ctx: &MemRegCtx,
    particle: Point,
    location: ParticleLocation,
    star_api: HyperStarApi<MemCosmos>,
) -> Result<(), CosmicErr> {
    let details = Details::new(
        Stub {
            point: particle.clone(),
            kind: Kind::Control,
            status: Status::Ready,
        },
        Properties::new(),
    );
    ctx.particles.insert(
        particle.clone(),
        ParticleRecord::new(details.clone(), location),
    );

    let mut wave = DirectedProto::ping();
    wave.to(star_api.get_skel().await?.point.clone().to_surface());
    wave.from(HYPERUSER.clone());
    wave.agent(Agent::HyperUser);
    wave.method(HypMethod::Assign);
    wave.body(Substance::Hyper(HyperSubstance::Assign(Assign::new(
        AssignmentKind::Create,
        details,
        StateSrc::None,
    ))));
    let wave = wave.build()?;
    let wave = wave.to_ultra();
    star_api.to_gravity(wave).await;
    Ok(())
}

#[test]
fn test_control() -> Result<(), CosmicErr> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        // let (final_tx, final_rx) = oneshot::channel();

        let platform = MemCosmos::new();
        let machine_api = platform.machine();
        let logger = RootLogger::new(LogSource::Core, Arc::new(StdOutAppender()));
        let logger = logger.point(Point::from_str("mem-client").unwrap());

        tokio::time::timeout(Duration::from_secs(10), machine_api.wait_ready())
            .await
            .unwrap();

        let factory = MachineApiExtFactory {
            machine_api,
            logger: logger.clone(),
        };

        let exchanger = Exchanger::new(
            Point::from_str("client").unwrap().to_surface(),
            Timeouts::default(),
            Default::default(),
        );
        let client =
            HyperClient::new_with_exchanger(Box::new(factory), Some(exchanger), logger).unwrap();
        let transmitter = client.transmitter_builder().await?;
        let greet = client.get_greeting().expect("expected greeting");
        let transmitter = transmitter.build();

        {
            let transmitter = transmitter.clone();
            let mut rx = client.rx();
            tokio::spawn(async move {
                while let Ok(wave) = rx.recv().await {
                    if wave.is_directed() {
                        let directed = wave.to_directed().unwrap();
                        if directed.core().method == Method::Cmd(CmdMethod::Bounce) {
                            let reflection = directed.reflection().unwrap();
                            let reflect =
                                reflection.make(ReflectedCore::ok(), greet.surface.clone());
                            let wave = reflect.to_ultra();
                            transmitter.route(wave).await;
                        }
                    }
                }
            });
        }

        let mut bounce = DirectedProto::cmd(
            greet.transport.clone().with_layer(Layer::Shell),
            CmdMethod::Bounce,
        );
        let reflect: Wave<Pong> = transmitter.direct(bounce).await?;

        assert!(reflect.core.status.is_success());

        client.close().await;
        tokio::time::sleep(Duration::from_millis(50)).await;
        Ok(())
    })
}

#[test]
fn test_star_wrangle() -> Result<(), CosmicErr> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        // let (final_tx, final_rx) = oneshot::channel();

        let platform = MemCosmos::new();
        let machine_api = platform.machine();
        let logger = RootLogger::new(LogSource::Core, Arc::new(StdOutAppender()));

        tokio::time::timeout(Duration::from_secs(1), machine_api.wait_ready())
            .await
            .unwrap();

        let star_api = machine_api.get_machine_star().await?;

        let wrangles = tokio::time::timeout(Duration::from_secs(55), star_api.wrangle())
            .await
            .unwrap()
            .unwrap();

        println!("wrangles: {}", wrangles.wrangles.len());

        for kind in wrangles.wrangles.iter() {
            println!("\tkind: {}", kind.key().to_string());
        }

        Ok(())
    })
}

#[test]
fn test_golden_path() -> Result<(), CosmicErr> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        // let (final_tx, final_rx) = oneshot::channel();

        let platform = MemCosmos::new();
        let machine_api = platform.machine();

        tokio::time::timeout(Duration::from_secs(1), machine_api.wait_ready())
            .await
            .unwrap();

        let fold = StarKey::new(&"central".to_string(), &StarHandle::new("fold", 0));
        let star_api = machine_api.get_star(fold).await?;

        // first mem if we can bounce nexus which fold should be directly connected too
        let nexus = StarKey::new(&"central".to_string(), &StarHandle::new("nexus", 0));
        tokio::time::timeout(Duration::from_secs(5), star_api.bounce(nexus)).await??;
        println!("Ok");

        // this one should require a search operation in order to find
        tokio::time::timeout(Duration::from_secs(5), star_api.bounce(StarKey::central())).await??;

        println!("Ok");

        Ok(())
    })
}

#[test]
fn test_provision_and_assign() -> Result<(), CosmicErr> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        // let (final_tx, final_rx) = oneshot::channel();

        let platform = MemCosmos::new();
        let machine_api = platform.machine();
        let logger = RootLogger::new(LogSource::Core, Arc::new(StdOutAppender()));
        let logger = logger.point(Point::from_str("mem-client").unwrap());

        tokio::time::timeout(Duration::from_secs(5), machine_api.wait_ready())
            .await
            .unwrap();

        let factory = MachineApiExtFactory {
            machine_api,
            logger: logger.clone(),
        };

        let client = ControlClient::new(Box::new(factory))?;
        client.wait_for_ready(Duration::from_secs(5)).await?;

        let transmitter = client.transmitter_builder().await?;
        let transmitter = transmitter.build();

        tokio::time::sleep(Duration::from_secs(5)).await;
        assert!(
            transmitter
                .bounce(&Point::global_executor().to_surface())
                .await
        );

        let create = Create {
            template: Template::new(
                PointTemplate {
                    parent: Point::root(),
                    child_segment_template: PointSegTemplate::Exact("my-domain.com".to_string()),
                },
                Kind::Space.to_template(),
            ),
            properties: Default::default(),
            strategy: Strategy::Override,
            state: StateSrc::None,
        };
        let mut proto: DirectedProto = create.into();
        //proto.track = true;

        let reflect: Wave<Pong> = transmitter.direct(proto).await?;
        assert!(reflect.core.is_ok());

        tokio::time::sleep(Duration::from_secs(5)).await;

        let point = Point::from_str("my-domain.com")?;
        let mut proto = DirectedProto::ping();
        proto.method(CmdMethod::Bounce);
        proto.to(point.to_surface());
        let reflect: Wave<Pong> = transmitter.direct(proto).await?;
        println!("{}", reflect.core.status.to_string());
        assert!(reflect.core.is_ok());

        Ok(())
    })
}

#[test]
fn test_control_cli() -> Result<(), CosmicErr> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        // let (final_tx, final_rx) = oneshot::channel();

        let platform = MemCosmos::new();
        let machine_api = platform.machine();
        let logger = RootLogger::new(LogSource::Core, Arc::new(StdOutAppender()));
        let logger = logger.point(Point::from_str("mem-client").unwrap());

        tokio::time::timeout(Duration::from_secs(3), machine_api.wait_ready())
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_secs(5)).await;

        let factory = MachineApiExtFactory {
            machine_api,
            logger: logger.clone(),
        };

        let client = ControlClient::new(Box::new(factory))?;
        client.wait_for_ready(Duration::from_secs(5)).await?;

        let cli = client.new_cli_session().await?;

        let core = cli.exec("create localhost<Space>").await?;

        println!("{}", core.to_err().to_string());
        assert!(core.is_ok());
        let core = cli.exec("create localhost:base<Base>").await?;
        assert!(core.is_ok());

        Ok(())
    })
}

#[test]
fn test_publish() -> Result<(), CosmicErr> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        // let (final_tx, final_rx) = oneshot::channel();

        let cosmos = MemCosmos::new();
        let machine_api = cosmos.machine();
        let logger = RootLogger::new(LogSource::Core, Arc::new(StdOutAppender()));
        let logger = logger.point(Point::from_str("mem-client").unwrap());

        tokio::time::timeout(Duration::from_secs(10), machine_api.wait_ready())
            .await
            .unwrap();

        let factory = MachineApiExtFactory {
            machine_api,
            logger: logger.clone(),
        };

        let client = ControlClient::new(Box::new(factory))?;
        client.wait_for_ready(Duration::from_secs(5)).await?;

        tokio::time::sleep(Duration::from_secs(1)).await;

        let cli = client.new_cli_session().await?;

        logger
            .result(cli.exec("create localhost<Space>").await.unwrap().ok_or())
            .unwrap();
        cli.exec("create localhost:repo<Repo>")
            .await
            .unwrap()
            .ok_or()
            .unwrap();
        cli.exec("create localhost:repo:my<BundleSeries>")
            .await
            .unwrap()
            .ok_or()
            .unwrap();

        let mut command = RawCommand::new("publish ^[ bundle.zip ]-> localhost:repo:my:1.0.0");

        let file_path = "test/bundle.zip";
        let bin = Arc::new(fs::read(file_path)?);
        command.transfers.push(CmdTransfer::new("bundle.zip", bin));

        let core = cli.raw(command).await?;

        if !core.is_ok() {
            if let Substance::FormErrs(ref e) = core.body {
                println!("{}", e.to_string());
            }
        }

        assert!(core.is_ok());

        Ok(())
    })
}

//#[test]
fn test_mechtron() -> Result<(), CosmicErr> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        // let (final_tx, final_rx) = oneshot::channel();

        let cosmos = MemCosmos::new();
        let machine_api = cosmos.machine();
        let logger = RootLogger::new(LogSource::Core, Arc::new(StdOutAppender()));
        let logger = logger.point(Point::from_str("mem-client").unwrap());

        tokio::time::timeout(Duration::from_secs(10), machine_api.wait_ready())
            .await
            .unwrap();

        let factory = MachineApiExtFactory {
            machine_api,
            logger: logger.clone(),
        };

        let client = ControlClient::new(Box::new(factory))?;
        client.wait_for_ready(Duration::from_secs(5)).await?;

        tokio::time::sleep(Duration::from_secs(1)).await;


        let cli = client.new_cli_session().await?;

        cli.exec("create repo<Repo>")
            .await
            .unwrap()
            .ok_or()
            .unwrap();
        cli.exec("create repo:hello-goodbye<BundleSeries>")
            .await
            .unwrap()
            .ok_or()
            .unwrap();

        let mut command = RawCommand::new("publish ^[ bundle.zip ]-> repo:hello-goodbye:1.0.0");

        let file_path = "../../mech-old/mocks/hello-goodbye/bundle.zip";
        let bin = Arc::new(fs::read(file_path)?);
        command.transfers.push(CmdTransfer::new("bundle.zip", bin));

        let core = cli.raw(command).await?;

        if !core.is_ok() {
            if let Substance::FormErrs(ref e) = core.body {
                println!("{}", e.to_string());
            }
        }

        assert!(core.is_ok());

        tokio::time::sleep(Duration::from_secs(1)).await;

       let reflect = cli.exec("create hello-goodbye<Mechtron>{ +config=repo:hello-goodbye:1.0.0:/config/hello-goodbye.mech-old }")
            .await
            .unwrap();

        assert!(reflect.is_ok());

        let mut proto = DirectedProto::ping();
        proto.to(Point::from_str("hello-goodbye")?.to_surface());
        proto.method(ExtMethod::new("Hello").unwrap());

        let transmitter = client.transmitter_builder().await.unwrap().build();
        let result = transmitter.ping(proto).await.unwrap();

        assert!(result.is_ok());

        if let Substance::Text(text) = &result.core.body {
            assert_eq!(text.as_str(), "Goodbye")
        } else {
            assert!(false);
        }

        Ok(())
    })
}

pub async fn verify<S>(name: &str, ser: &S)
where
    S: Serialize,
{
    let bin = bincode::serialize(&ser).unwrap();
    fs::create_dir(Path::new("e2e"));
    let file = format!("e2e/{}", name);
    let path = Path::new(file.as_str());
    if path.exists() == true {
        if fs::read(path).unwrap() != bin {
            assert!(false)
        }
    } else {
        fs::write(path, bin).unwrap();
    }
}

#[test]
fn test_create_err() -> Result<(), CosmicErr> {
    #[derive(Copy, Clone)]
    pub struct CreateErrTest;
    #[async_trait]
    impl Test for CreateErrTest {
        async fn run(&self, client: ControlClient) -> Result<(), CosmicErr> {
            let cli = client.new_cli_session().await?;
            if let Err(err) = cli.exec("create repo<BadKind>").await?.ok_or() {
                verify("create_err", &err).await;
                println!("FINAL: ");
                err.print();
                Ok(())
            } else {
                Err("expected err".into())
            }
        }
    }

    harness(CreateErrTest)
}
