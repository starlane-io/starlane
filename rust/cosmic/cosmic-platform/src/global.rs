use cosmic_api::cli::RawCommand;
use cosmic_api::error::MsgErr;
use cosmic_api::parse::command_line;
use cosmic_api::parse::error::result;
use cosmic_api::util::{log, ToResolved};
use cosmic_api::wave::{InCtx, ProtoTransmitter, ReflectedCore};
use cosmic_nom::new_span;
use crate::{Platform, Registry};
/*

#[derive(DirectedHandler)]
pub struct Global<P> where P: Platform {
    pub registry: Registry<P>,
    pub transmitter: ProtoTransmitter
}
use cosmic_api::wave::DirectedHandlerSelector;
use cosmic_api::wave::RecipientSelector;
use cosmic_api::wave::RootInCtx;
use cosmic_api::wave::CoreBounce;

#[routes]
impl <P> Global<P> where P: Platform {

    /*
    #[route("Cmd<Command>")]
    pub async fn command( &self, ctx: InCtx<'_,RawCommand> ) -> Result<ReflectedCore,MsgErr> {
        let span = new_span(ctx.input.line.as_str() );
        let command = log(result(command_line(span )))?;
        let command = command.collapse()?;
    }

     */
}

 */