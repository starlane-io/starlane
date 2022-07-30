use std::sync::Arc;
use cosmic_api::cli::RawCommand;
use cosmic_api::command::Command;
use cosmic_api::command::request::create::{Create, PointSegTemplate, Strategy};
use cosmic_api::error::MsgErr;
use cosmic_api::id::id::{GLOBAL_EXEC, Kind, Point, Port, ToPort};
use cosmic_api::log::{PointLogger, RootLogger};
use cosmic_api::parse::command_line;
use cosmic_api::parse::error::result;
use cosmic_api::particle::particle::Details;
use cosmic_api::Registration;
use cosmic_api::util::{log, ToResolved};
use cosmic_api::wave::{Agent, DirectedHandlerShell, Exchanger, Handling, InCtx, ProtoTransmitter, ProtoTransmitterBuilder, ReflectedCore, Router, Scope, SetStrategy};
use cosmic_nom::new_span;
use crate::{PlatErr, Platform, Registry};

/*
#[derive(DirectedHandler,Clone)]
pub struct Global<P> where P: Platform {
    pub logger: PointLogger,
    pub registry: Registry<P>,
}

 */


use cosmic_api::wave::DirectedHandlerSelector;
use cosmic_api::wave::RecipientSelector;
use cosmic_api::wave::RootInCtx;
use cosmic_api::wave::CoreBounce;
use cosmic_api::wave::DirectedHandler;
use cosmic_api::config::config::bind::RouteSelector;
use cosmic_api::parse::route_attribute;
use cosmic_api::sys::Assign;
use crate::driver::{Driver, Item, ItemHandler};
use crate::star::StarSkel;


#[derive(DirectedHandler)]
pub struct GlobalDriver<P> where P: Platform {
   pub skel: StarSkel<P>,
   core_point: Point
}

#[async_trait]
impl <P> Driver<P> for GlobalDriver<P> where P: Platform{
    fn kind(&self) -> Kind {
        Kind::Global
    }

    async fn item(&self, point: &Point) -> Result<Box<dyn ItemHandler<P>>, P::Err> {
        if *point == self.core_point {
            Ok(Box::new(GlobalCore::restore(self.skel.clone(), (), () )))
        } else {
            Err(MsgErr::not_found().into())
        }
    }

    async fn assign(&self, assign: Assign) -> Result<(), MsgErr> {
        Err(MsgErr::forbidden())
    }
}

#[routes]
impl <P> GlobalDriver<P> where P: Platform {
   pub fn new(skel: StarSkel<P>) -> Self {
       let core_point = skel.point.push("global").unwrap();
       Self {
           skel,
           core_point
       }
   }
}


#[derive(DirectedHandler)]
pub struct GlobalCore<P> where P: Platform {
  skel: StarSkel<P>
}

impl<P> ItemHandler<P> for GlobalCore<P> where P: Platform {}

impl <P> Item<P> for GlobalCore<P> where P: Platform {
    type Skel = StarSkel<P>;
    type Ctx = ();
    type State = ();

    fn restore(skel: Self::Skel, ctx: Self::Ctx, state: Self::State) -> Self {
        GlobalCore {
            skel
        }
    }
}




#[routes]
impl <P> GlobalCore<P> where P: Platform {

    #[route("Cmd<RawCommand>")]
    pub async fn raw( &self, ctx: InCtx<'_,RawCommand> ) -> Result<ReflectedCore,P::Err> {
        let line = ctx.input.line.clone();
        let span = new_span(line.as_str() );
        let command = log(result(command_line(span )))?;
        let command = command.collapse()?;
        let ctx = ctx.push_input_ref(&command);
        self.command( ctx ).await
    }

    #[route("Cmd<Command>")]
    pub async fn command( &self, ctx: InCtx<'_,Command> ) -> Result<ReflectedCore,P::Err> {
        match ctx.input {
            Command::Create(create) => {
                Ok(ctx.ok_body(self.create(&create).await?.stub.into()))
            }
            _ => {
                Err(P::Err::new("not implemented"))
            }
        }
    }


    pub async fn create(&self, create: &Create) -> Result<Details, P::Err> {
        let child_kind = self.skel.machine.platform.default_implementation(&create.template.kind)?;
        let stub = match &create.template.point.child_segment_template {
            PointSegTemplate::Exact(child_segment) => {
                let point = create.template.point.parent.push(child_segment.clone());
                match &point {
                    Ok(_) => {}
                    Err(err) => {
                        eprintln!("RC CREATE error: {}", err.to_string());
                    }
                }
                let point = point?;

                let properties = self.skel.machine.platform.properties_config(&child_kind)
                    .fill_create_defaults(&create.properties)?;
                self.skel.machine.platform.properties_config(&child_kind).check_create(&properties)?;

                let registration = Registration {
                    point: point.clone(),
                    kind: child_kind.clone(),
                    registry: create.registry.clone(),
                    properties,
                    owner: Point::root(),
                    strategy: create.strategy.clone()
                };
                println!("creating {}", point.to_string());
                let mut result = self.skel.registry.register(&registration).await;

                println!("result {}? {}", point.to_string(), result.is_ok());
                result?
            }
            PointSegTemplate::Pattern(pattern) => {
                if !pattern.contains("%") {
                    return Err(P::Err::status_msg(500u16, "AddressSegmentTemplate::Pattern must have at least one '%' char for substitution"));
                }
                loop {
                    let index = self.skel.registry.sequence(&create.template.point.parent).await?;
                    let child_segment = pattern.replace("%", index.to_string().as_str());
                    let point = create.template.point.parent.push(child_segment.clone())?;
                    let registration = Registration {
                        point: point.clone(),
                        kind: child_kind.clone(),
                        registry: create.registry.clone(),
                        properties: create.properties.clone(),
                        owner: Point::root(),
                        strategy: create.strategy.clone()
                    };

                    self.skel.registry.register(&registration).await?;
                }
            }
        };
        Ok(stub)
    }


    }

