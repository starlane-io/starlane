# COSMIC MACROS
`cosmic-macros` is one of the packages that compose [THE COSMIC INITIATIVE](http://thecosmicinitiative.io) a WebAssembly orchestration framework.

### DirectedHandler
Derive the `DirectedHandler` to receive Waves:
```rust
#[derive(DirectedHandler)]
pub struct MyHandler {
  logger: PointLogger
}
```

### ROUTES
Flag one and only one impl with `#[routes]` and annotate functions
functions with `#route[()]` in order to select messages:
```rust
#[routes]
impl MyHandler {
   #[route("Ext<MyNameIs>")]
   pub async fn hello(&self, ctx: InCtx<'_, Text>) -> Result<String, UniErr> {
     Ok(format!("Hello, {}", ctx.input.to_string()))
   }
}
```

### FULL EXAMPLE

```rust
use cosmic_space::err::UniErr;
use cosmic_space::hyper::HyperSubstance;
use cosmic_space::log::PointLogger;
use cosmic_space::substance::Substance;
use cosmic_space::substance::Substance::Text;
use cosmic_space::wave::core::ReflectedCore;
use cosmic_space::wave::exchange::InCtx;

#[derive(DirectedHandler)]
pub struct MyHandler {
  logger: PointLogger
}

#[routes]
impl MyHandler {
   /// the route attribute captures an ExtMethod implementing a custom `MyNameIs`
   /// notice that the InCtx will accept any valid cosmic_space::substance::Substance
   #[route("Ext<MyNameIs>")]
   pub async fn hello(&self, ctx: InCtx<'_, Text>) -> Result<String, UniErr> {
     /// also we can return any Substance in our Reflected wave
     Ok(format!("Hello, {}", ctx.input.to_string()))
   }

   /// if the function returns nothing then an Empty Ok Reflected will be returned unless
   /// the wave type is `Wave<Signal>`
   #[route("Ext<Bye>")]
   pub async fn bye(&self, ctx: InCtx<'_,()>) {
     self.logger.info("funny that! He left without saying a word!");
   }
}
```

