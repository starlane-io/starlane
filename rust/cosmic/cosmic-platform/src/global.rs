use std::sync::Arc;
use cosmic_api::cli::RawCommand;
use cosmic_api::command::Command;
use cosmic_api::command::request::create::{Create, PointSegTemplate, Strategy};
use cosmic_api::error::MsgErr;
use cosmic_api::id::id::{Point, Port, ToPort};
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
/*
#[routes]
impl <P> Global<P> where P: Platform {

    pub fn new(registry: Registry<P>, router: Arc<dyn Router>, exchanger: Exchanger, port: Port, logger: PointLogger ) -> DirectedHandlerShell<Global<P>> {
        let mut builder = ProtoTransmitterBuilder::new( router, exchanger );
        builder.from = SetStrategy::Override(port.clone());
        builder.agent = SetStrategy::Fill(Agent::HyperUser);
        builder.scope = SetStrategy::Fill(Scope::Full);
        builder.handling= SetStrategy::Fill(Handling::default());

        let global = Self{
            registry,
            logger: logger.clone()
        };

        DirectedHandlerShell::new( Box::new(global), builder, port, logger.logger  )
    }

    #[route("Cmd<RawCommand>")]
    pub async fn raw( &self, ctx: InCtx<'_,RawCommand> ) -> Result<ReflectedCore,MsgErr> {
        let span = new_span(ctx.input.line.as_str() );
        let command = log(result(command_line(span )))?;
        let command = command.collapse()?;
        self.command( command ).await
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
        let child_kind = match_kind(&create.template.kind)?;
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

                let properties = properties_config(&child_kind)
                    .fill_create_defaults(&create.properties)?;
                properties_config(&child_kind).check_create(&properties)?;

                let registration = Registration {
                    point: point.clone(),
                    kind: child_kind.clone(),
                    registry: create.registry.clone(),
                    properties,
                    owner: Point::root(),
                };
                println!("creating {}", point.to_string());
                let mut result = self.registry.register(&registration).await;

                // if strategy is ensure then a dupe is GOOD!
                if create.strategy == Strategy::Ensure {
                    if let Err(RegError::Dupe) = result {
                        result = Ok(self.locate(&point).await?.details);
                    }
                }

                println!("result {}? {}", point.to_string(), result.is_ok());
                result?
            }
            PointSegTemplate::Pattern(pattern) => {
                if !pattern.contains("%") {
                    return Err("AddressSegmentTemplate::Pattern must have at least one '%' char for substitution".into());
                }
                loop {
                    let index = self.sequence(&create.template.point.parent).await?;
                    let child_segment = pattern.replace("%", index.to_string().as_str());
                    let point = create.template.point.parent.push(child_segment.clone())?;
                    let registration = Registration {
                        point: point.clone(),
                        kind: child_kind.clone(),
                        registry: create.registry.clone(),
                        properties: create.properties.clone(),
                        owner: Point::root(),
                    };

                    match self.registry.register(&registration).await {
                        Ok(stub) => {
                            return Ok(stub)
                        }
                        Err(RegError::Dupe) => {
                            // continue loop
                        }
                        Err(RegError::Error(error)) => {
                            return Err(error);
                        }
                    }
                }
            }
        };
        Ok(stub)
    }


    }
 */

