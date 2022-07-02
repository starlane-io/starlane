impl DirectedHandlerSelector for DriverShell {
    fn select<'a>(&self, select: &'a RecipientSelector<'a>) -> Result<&dyn DirectedHandler, ()> {
        if __DriverShell_assign__.is_match(&select.wave).is_ok() {
            return Ok(self);
        }
        Err(())
    }
}
#[async_trait]
impl DirectedHandler for DriverShell {
    async fn handle(&self, ctx: RootInCtx) -> Bounce {
        if __DriverShell_assign__.is_match(&ctx.wave).is_ok() {
            return self.__assign__route(ctx).await;
        }
        Bounce::Reflect(*ctx.not_found().core())
    }
}
lazy_static! {
    static ref __DriverShell_assign__: RouteSelector =
        mesh_portal::version::latest::parse::route_attribute("#[route(\"Sys<Assign>\")]").unwrap();
}
async fn __assign__route(&self, mut ctx: RootInCtx) -> Bounce {
    let ctx: InCtx<'_, Sys> = match ctx.push() {
        Ok(ctx) => ctx,
        Err(err) => {
            return Bounce::Reflect(ReflectedCore::server_error());
        }
    };
    match self.assign(ctx).await {
        Ok(rtn) => Bounce::Reflect(rtn.into()),
        Err(err) => Bounce(ReflectedCore::server_error()),
    }
}
async fn assign(&self, ctx: InCtx<'_, Sys>) -> Result<ReflectedCore, MsgErr> {
    match ctx.input {
        Sys::Assign(assign) => {
            let ctx = ctx.push_input_ref(assign);
            self.core.assign(ctx).await
        }
        _ => Err(MsgErr::bad_request()),
    }
}
