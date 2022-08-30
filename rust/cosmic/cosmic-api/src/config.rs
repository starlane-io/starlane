pub mod config {
    use std::collections::HashMap;
    use std::ops::Deref;
    use std::pin::Pin;

    use serde::{Deserialize, Serialize};

    use crate::config::config::bind::BindConfig;
    use crate::id::id::{KindParts, Point};
    use crate::parse::model::{MethodScope, RouteScope, WaveScope};
    use crate::particle::particle;
    use crate::particle::particle::{Details, Stub};
    use crate::util::ValueMatcher;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum PortalKind {
        Mechtron,
        Portal,
    }

    impl ToString for PortalKind {
        fn to_string(&self) -> String {
            match self {
                PortalKind::Mechtron => "Mechtron".to_string(),
                PortalKind::Portal => "Portal".to_string(),
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Info {
        pub stub: Stub,
        pub kind: PortalKind,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct PortalConfig {
        pub max_payload_size: u32,
        pub init_timeout: u64,
        pub frame_timeout: u64,
        pub response_timeout: u64,
    }

    impl Default for PortalConfig {
        fn default() -> Self {
            Self {
                max_payload_size: 128 * 1024,
                init_timeout: 30,
                frame_timeout: 5,
                response_timeout: 15,
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
    pub struct PointConfig<Body> {
        pub point: Point,
        pub body: Body,
    }

    impl<Body> Deref for PointConfig<Body> {
        type Target = Body;

        fn deref(&self) -> &Self::Target {
            &self.body
        }
    }

    #[derive(Clone)]
    pub enum Document {
        BindConfig(BindConfig),
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
    pub struct ParticleConfigBody {
        pub details: Details,
    }

    pub mod bind {
        use crate::command::request::Rc;
        use crate::error::{MsgErr, ParseErrs};
        use crate::id::id::{Point, PointCtx, PointVar, Topic};
        use crate::substance::substance::{Call, CallDef};
        use crate::substance::substance::{Substance, SubstancePattern};
        use crate::wave::{
            DirectedCore, DirectedWave, MethodKind, MethodPattern, Ping, RecipientSelector,
            SingularDirectedWave, Wave,
        };

        use crate::parse::model::{
            BindScope, MethodScope, PipelineSegment, PipelineSegmentDef, PipelineVar, RouteScope,
            ScopeFilters, WaveScope,
        };
        use crate::parse::{bind_config, Env};
        use crate::selector::{PayloadBlock, PayloadBlockDef};
        use crate::util::{ToResolved, ValueMatcher, ValuePattern};
        use regex::Regex;
        use serde::{Deserialize, Serialize};
        use std::convert::TryInto;

        #[derive(Debug,Clone,Serialize,Deserialize)]
        pub enum WaveDirection {
            Direct,
            Reflect
        }

        #[derive(Clone)]
        pub struct BindConfig {
            scopes: Vec<BindScope>, /*pub msg: ConfigScope<EntityKind, Selector<MsgPipelineSelector>>,
                                    pub http: ConfigScope<EntityKind, Selector<HttpPipelineSelector>>,
                                    pub rc: ConfigScope<EntityKind, Selector<RcPipelineSelector>>,

                                     */
        }

        impl BindConfig {
            pub fn new(scopes: Vec<BindScope>) -> Self {
                Self { scopes }
            }

            pub fn route_scopes(&self) -> Vec<&RouteScope> {
                let mut scopes = vec![];
                for scope in &self.scopes {
                    if let BindScope::RequestScope(request_scope) = &scope {
                        scopes.push(request_scope);
                    }
                }
                scopes
            }

            pub fn select(&self, directed: &DirectedWave) -> Result<&MethodScope, MsgErr> {
                for route_scope in self.route_scopes() {
                    if route_scope.selector.is_match(directed).is_ok() {
                        for message_scope in &route_scope.block {
                            if message_scope.selector.is_match(directed).is_ok() {
                                for method_scope in &message_scope.block {
                                    if method_scope.selector.is_match(directed).is_ok() {
                                        return Ok(method_scope);
                                    }
                                }
                            }
                        }
                    }
                }
                Err(MsgErr::Status{
                    status: 404,
                    message: format!("no route matches {}<{}>{}", directed.kind().to_string(), directed.core().method.to_string(), directed.core().uri.path().to_string())
                })
            }
        }

        impl TryFrom<Vec<u8>> for BindConfig {
            type Error = MsgErr;

            fn try_from(doc: Vec<u8>) -> Result<Self, Self::Error> {
                let doc = String::from_utf8(doc)?;
                bind_config(doc.as_str())
            }
        }

        pub struct Cursor {}

        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub struct ConfigScope<T, E> {
            pub scope_type: T,
            pub elements: Vec<E>,
        }

        impl<T, E> ConfigScope<T, E> {
            pub fn new(scope_type: T, elements: Vec<E>) -> Self {
                Self {
                    scope_type,
                    elements,
                }
            }
        }

        pub type Pipeline = PipelineDef<Point>;
        pub type PipelineCtx = PipelineDef<PointCtx>;

        #[derive(Debug, Clone)]
        pub struct PipelineDef<Pnt> {
            pub segments: Vec<PipelineSegmentDef<Pnt>>,
        }

        impl<Pnt> PipelineDef<Pnt> {
            pub fn new() -> Self {
                Self { segments: vec![] }
            }

            pub fn consume(&mut self) -> Option<PipelineSegmentDef<Pnt>> {
                if self.segments.is_empty() {
                    Option::None
                } else {
                    Option::Some(self.segments.remove(0))
                }
            }
        }

        pub type PipelineStepVar = PipelineStepDef<PointVar>;
        pub type PipelineStepCtx = PipelineStepDef<PointCtx>;
        pub type PipelineStep = PipelineStepDef<Point>;

        #[derive(Debug, Clone)]
        pub struct PipelineStepDef<Pnt> {
            pub entry: WaveDirection,
            pub exit: WaveDirection,
            pub blocks: Vec<PayloadBlockDef<Pnt>>,
        }

        impl <Pnt> PipelineStepDef<Pnt> {
            pub fn direct() -> Self {
                Self {
                    entry: WaveDirection::Direct,
                    exit: WaveDirection::Direct,
                    blocks: vec![]
                }
            }

            pub fn rtn() -> Self {
                Self {
                    entry: WaveDirection::Reflect,
                    exit: WaveDirection::Reflect,
                    blocks: vec![]
                }
            }
        }

        impl ToResolved<PipelineStep> for PipelineStepCtx {
            fn to_resolved(self, env: &Env) -> Result<PipelineStep, MsgErr> {
                let mut blocks = vec![];
                for block in self.blocks {
                    blocks.push(block.to_resolved(env)?);
                }

                Ok(PipelineStep {
                    entry: self.entry,
                    exit: self.exit,
                    blocks,
                })
            }
        }

        impl ToResolved<PipelineStepCtx> for PipelineStepVar {
            fn to_resolved(self, env: &Env) -> Result<PipelineStepCtx, MsgErr> {
                let mut blocks = vec![];
                for block in self.blocks {
                    blocks.push(block.to_resolved(env)?);
                }

                Ok(PipelineStepCtx {
                    entry: self.entry,
                    exit: self.exit,
                    blocks,
                })
            }
        }

        /*
        impl CtxSubst<PipelineStep> for PipelineStepCtx{
            fn resolve_ctx(self, resolver: &dyn CtxResolver) -> Result<PipelineStep, MsgErr> {
                let mut errs = vec![];
                let mut blocks = vec![];
                for block in self.blocks {
                    match block.resolve_ctx(resolver) {
                        Ok(block)=>blocks.push(block),
                        Err(err)=>errs.push(err)
                    }
                }
                if errs.is_empty() {
                    Ok(PipelineStep{
                        entry:self.entry,
                        exit: self.exit,
                        blocks
                    })
                } else {
                    Err(ParseErrs::fold(errs).into())
                }
            }
        }

         */

        impl PipelineStep {
            pub fn new(entry: WaveDirection, exit: WaveDirection) -> Self {
                Self {
                    entry,
                    exit,
                    blocks: vec![],
                }
            }
        }

        /*
        #[derive(Debug,Clone,Eq,PartialEq)]
        pub struct CreateBlock{
            pub payload: Payload
        }

         */

        pub type PatternBlock = ValuePattern<SubstancePattern>;

        pub type PipelineStopCtx = PipelineStopDef<PointCtx>;
        pub type PipelineStopVar = PipelineStopDef<PointVar>;
        pub type PipelineStop = PipelineStopDef<Point>;

        #[derive(Debug, Clone)]
        pub enum PipelineStopDef<Pnt> {
            Core,
            Call(CallDef<Pnt>),
            Reflect,
            Point(Pnt),
            Err{ status: u16, msg: String }
        }

        impl ToResolved<PipelineStop> for PipelineStopVar {
            fn to_resolved(self, env: &Env) -> Result<PipelineStop, MsgErr> {
                let stop: PipelineStopCtx = self.to_resolved(env)?;
                stop.to_resolved(env)
            }
        }

        impl ToResolved<PipelineStop> for PipelineStopCtx {
            fn to_resolved(self, env: &Env) -> Result<PipelineStop, MsgErr> {
                Ok(match self {
                    PipelineStopCtx::Core => PipelineStop::Core,
                    PipelineStopCtx::Call(call) => PipelineStop::Call(call.to_resolved(env)?),
                    PipelineStopCtx::Reflect => PipelineStop::Reflect,
                    PipelineStopCtx::Point(point) => PipelineStop::Point(point.to_resolved(env)?),
                    PipelineStopCtx::Err { status, msg } => PipelineStop::Err{ status, msg }
                })
            }
        }

        impl ToResolved<PipelineStopCtx> for PipelineStopVar {
            fn to_resolved(self, env: &Env) -> Result<PipelineStopCtx, MsgErr> {
                Ok(match self {
                    PipelineStopVar::Core => PipelineStopCtx::Core,
                    PipelineStopVar::Call(call) => PipelineStopCtx::Call(call.to_resolved(env)?),
                    PipelineStopVar::Reflect => PipelineStopCtx::Reflect,
                    PipelineStopVar::Point(point) => {
                        PipelineStopCtx::Point(point.to_resolved(env)?)
                    }
                    PipelineStopVar::Err { status, msg } => PipelineStopCtx::Err{ status, msg }
                })
            }
        }

        /*
        impl CtxSubst<PipelineStop> for PipelineStopCtx {
            fn resolve_ctx(self, resolver: &dyn CtxResolver) -> Result<PipelineStop, MsgErr> {
                match self {
                    PipelineStopCtx::Internal => Ok(PipelineStop::Internal),
                    PipelineStopCtx::Call(call) => Ok(PipelineStop::Call(call.resolve_ctx(resolver)?)),
                    PipelineStopCtx::Respond => Ok(PipelineStop::Respond),
                    PipelineStopCtx::Point(point) => Ok(PipelineStop::Point(point.resolve_ctx(resolver)?))
                }
            }
        }

         */

        pub enum Whitelist {
            Any,
            None,
            Enumerated(Vec<CallPattern>),
        }

        pub enum CallPattern {
            Any,
            Call,
        }


        #[derive(Clone)]
        pub struct RouteSelector {
            pub topic: Option<ValuePattern<Topic>>,
            pub method: ValuePattern<MethodPattern>,
            pub path: Regex,
            pub filters: ScopeFilters,
        }

        impl RouteSelector {
            pub fn new(
                topic: Option<ValuePattern<Topic>>,
                method: ValuePattern<MethodPattern>,
                path: Regex,
                filters: ScopeFilters,
            ) -> Self {
                Self {
                    topic,
                    method,
                    path,
                    filters,
                }
            }

            pub fn any() -> Self {
                Self {
                    topic: None,
                    method: ValuePattern::Any,
                    path: Regex::new("/.*").unwrap(),
                    filters: Default::default(),
                }
            }

            pub fn from_method(method: ValuePattern<MethodPattern>) -> Self {
                Self {
                    topic: None,
                    method,
                    path: Regex::new("/.*").unwrap(),
                    filters: Default::default(),
                }
            }

            pub fn with_topic(self, topic: Topic) -> Self {
                Self {
                    topic: Some(ValuePattern::Pattern(topic)),
                    method: self.method,
                    path: self.path,
                    filters: self.filters,
                }
            }

            pub fn is_match<'a>(&self, wave: &'a DirectedWave) -> Result<(), ()> {
                self.method.is_match(&wave.core().method)?;
                match self.path.is_match(&wave.core().uri.path()) {
                    true => Ok(()),
                    false => Err(()),
                }
            }
        }
    }
}
