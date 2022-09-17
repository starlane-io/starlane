use std::sync::Arc;

pub mod substance {
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;
    use std::ops::{Deref, DerefMut};

    use crate::substance2::Bin;
    use crate::command::RawCommand;
    use crate::command::request::{Rc, RcCommandType};
    use crate::command::Command;
    use crate::error::{ParseErrs, UniErr};
    use crate::http::HttpMethod;
    use crate::log::Log;
    use crate::ext::ExtMethod;
    use crate::parse::model::Subst;
    use crate::parse::{CtxResolver, Env};
    use crate::selector2::selector::{KindSelector, Selector};
    use crate::hyper::{Greet, HyperSubstance, Knock};
    use crate::util::{ToResolved, uuid, ValueMatcher, ValuePattern};
    use crate::wave::{
        CmdMethod, DirectedCore, HyperWave, HypMethod, Method, Pong, ReflectedCore, UltraWave, Wave,
    };
    use cosmic_macros_primitive::Autobox;
    use cosmic_nom::Tw;
    use http::header::CONTENT_TYPE;
    use http::{HeaderMap, HeaderValue, Uri};
    use serde_json::Value;
    use std::str::FromStr;
    use std::sync::Arc;
    use crate::particle::Details;
    use crate::id::{BaseKind, KindParts, Meta, Point, PointCtx, PointVar, Port};
    use crate::particle::{Particle, Status, Stub};

    #[derive(
        Debug,
        Clone,
        Serialize,
        Deserialize,
        Eq,
        PartialEq,
        strum_macros::Display,
        strum_macros::EnumString,
    )]
    pub enum SubstanceKind {
        Empty,
        List,
        Map,
        Point,
        Port,
        Text,
        Boolean,
        Int,
        Meta,
        Bin,
        Stub,
        Details,
        Status,
        Particle,
        Errors,
        Json,
        MultipartForm,
        RawCommand,
        Command,
        ReqCore,
        RespCore,
        Hyp,
        Token,
        UltraWave,
        HyperWave,
        Knock,
        Greet,
    }

    #[derive(
        Debug,
        Clone,
        Serialize,
        Deserialize,
        Eq,
        PartialEq,
        strum_macros::Display,
        Autobox,
        cosmic_macros_primitive::ToSubstance,
    )]
    pub enum Substance {
        Empty,
        List(SubstanceList),
        Map(SubstanceMap),
        Point(Point),
        Port(Port),
        Text(String),
        Stub(Stub),
        Details(Details),
        Meta(Meta),
        Bin(Bin),
        Boolean(bool),
        Int(i64),
        Status(Status),
        Particle(Particle),
        RawCommand(RawCommand),
        Command(Box<Command>),
        Errors(Errors),
        Json(Value),
        MultipartForm(MultipartForm),
        ReqCore(Box<DirectedCore>),
        RespCore(Box<ReflectedCore>),
        Hyper(HyperSubstance),
        Token(Token),
        UltraWave(Box<UltraWave>),
        HyperWave(Box<HyperWave>),
        Knock(Knock),
        Greet(Greet),
    }

    pub trait ToSubstance<S> {
        fn to_substance(self) -> Result<S, UniErr>;
        fn to_substance_ref(&self) -> Result<&S, UniErr>;
    }

    pub trait ChildSubstance {}

    #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
    pub struct Token {
        data: String,
    }

    impl Token {
        pub fn new<D: ToString>(data: D) -> Self {
            Self {
                data: data.to_string(),
            }
        }

        pub fn new_uuid() -> Self {
            Self::new(uuid())
        }
    }

    impl Deref for Token {
        type Target = String;

        fn deref(&self) -> &Self::Target {
            &self.data
        }
    }

    impl ToString for Token {
        fn to_string(&self) -> String {
            self.data.clone()
        }
    }

    impl FromStr for Token {
        type Err = UniErr;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            Ok(Token::new(s))
        }
    }

    impl TryFrom<Pong> for Token {
        type Error = UniErr;

        fn try_from(response: Pong) -> Result<Self, Self::Error> {
            response.core.body.try_into()
        }
    }

    pub trait ToRequestCore {
        type Method;
        fn to_request_core(self) -> DirectedCore;
    }

    impl Default for Substance {
        fn default() -> Self {
            Substance::Empty
        }
    }

    impl Substance {
        pub fn to_text(self) -> Result<String, UniErr> {
            if let Substance::Text(text) = self {
                Ok(text)
            } else {
                Err("not a 'Text' payload".into())
            }
        }

        pub fn is_some(&self) -> bool {
            if let Self::Empty = self {
                false
            } else {
                true
            }
        }

        pub fn from_bin(bin: Bin) -> Self {
            Self::Bin(bin)
        }

        pub fn kind(&self) -> SubstanceKind {
            match self {
                Substance::Empty => SubstanceKind::Empty,
                Substance::List(list) => SubstanceKind::List,
                Substance::Map(map) => SubstanceKind::Map,
                Substance::Point(_) => SubstanceKind::Point,
                Substance::Text(_) => SubstanceKind::Text,
                Substance::Stub(_) => SubstanceKind::Stub,
                Substance::Meta(_) => SubstanceKind::Meta,
                Substance::Bin(_) => SubstanceKind::Bin,
                Substance::Boolean(_) => SubstanceKind::Boolean,
                Substance::Int(_) => SubstanceKind::Int,
                Substance::Status(_) => SubstanceKind::Status,
                Substance::Particle(_) => SubstanceKind::Particle,
                Substance::Errors(_) => SubstanceKind::Errors,
                Substance::Json(_) => SubstanceKind::Json,
                Substance::RawCommand(_) => SubstanceKind::RawCommand,
                Substance::Port(_) => SubstanceKind::Port,
                Substance::Command(_) => SubstanceKind::Command,
                Substance::ReqCore(_) => SubstanceKind::ReqCore,
                Substance::RespCore(_) => SubstanceKind::RespCore,
                Substance::Hyper(_) => SubstanceKind::Hyp,
                Substance::MultipartForm(_) => SubstanceKind::MultipartForm,
                Substance::Token(_) => SubstanceKind::Token,
                Substance::UltraWave(_) => SubstanceKind::UltraWave,
                Substance::HyperWave(_) => SubstanceKind::HyperWave,
                Substance::Knock(_) => SubstanceKind::Knock,
                Substance::Greet(_) => SubstanceKind::Greet,
                Substance::Details(_) => SubstanceKind::Details
            }
        }

        pub fn to_bin(self) -> Result<Bin, UniErr> {
            match self {
                Substance::Empty => Ok(Arc::new(vec![])),
                Substance::List(list) => list.to_bin(),
                Substance::Map(map) => map.to_bin(),
                Substance::Bin(bin) => Ok(bin),
                _ => Err("not supported".into()),
            }
        }
    }

    impl TryInto<HashMap<String, Substance>> for Substance {
        type Error = UniErr;

        fn try_into(self) -> Result<HashMap<String, Substance>, Self::Error> {
            match self {
                Substance::Map(map) => Ok(map.map),
                _ => Err("Substance type must a Map".into()),
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
    pub struct SubstanceMap {
        pub map: HashMap<String, Substance>,
    }

    impl Deref for SubstanceMap {
        type Target = HashMap<String, Substance>;

        fn deref(&self) -> &Self::Target {
            &self.map
        }
    }

    impl DerefMut for SubstanceMap {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.map
        }
    }

    impl Default for SubstanceMap {
        fn default() -> Self {
            Self {
                map: Default::default(),
            }
        }
    }
    /*
    impl <ToKind,FromKind> TryInto<ToKind> for SubstanceMap<FromKind> {
        type Error = Error;

        fn try_into(self) -> Result<ToKind, Self::Error> {
            let mut map = HashMap::new();
            for (k,v) in self.map {
                map.insert( k, v.try_into()? );
            }
            Ok(Self{map})
        }
    }

     */

    impl SubstanceMap {
        /*
        pub fn new(constraints: MapConstraints<KEY,ADDRESS,IDENTIFIER,KIND> ) -> Self {
            Self{
        //        constraints,
                map: HashMap::new()
            }
        }

         */
        pub fn to_bin(self) -> Result<Bin, UniErr> {
            Ok(Arc::new(bincode::serialize(&self)?))
        }

        pub fn new() -> Self {
            Self {
                map: HashMap::new(),
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
    pub struct Errors {
        map: HashMap<String, String>,
    }



    impl Errors {
        pub fn to_cosmic_err(&self) -> UniErr {
            UniErr::new(500, self.to_string().as_str() )
        }

        pub fn empty() -> Self {
            Self {
                map: HashMap::new(),
            }
        }

        pub fn default<S:ToString>(message: S) -> Self {
            let mut map = HashMap::new();
            map.insert("default".to_string(), message.to_string());
            Self { map }
        }
    }

    impl From<UniErr> for Errors {
        fn from(err: UniErr) -> Self {
            match err {
                UniErr::Status { status, message } => {
                    Self::default(format!("{} {}", status, message).as_str())
                }
                UniErr::ParseErrs(_) => {
                    Self::default("500: parse error")
                }
            }
        }
    }

    impl ToString for Errors {
        fn to_string(&self) -> String {
            let mut rtn = String::new();
            for (index, (_, value)) in self.iter().enumerate() {
                rtn.push_str(value.as_str());
                if index == self.len() - 1 {
                    rtn.push_str("\n");
                }
            }
            rtn
        }
    }

    impl Deref for Errors {
        type Target = HashMap<String, String>;

        fn deref(&self) -> &Self::Target {
            &self.map
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
    pub struct SubstanceList {
        pub list: Vec<Box<Substance>>,
    }

    impl ToString for SubstanceList {
        fn to_string(&self) -> String {
            "[]".to_string()
        }
    }

    impl SubstanceList {
        pub fn new() -> Self {
            Self { list: vec![] }
        }
        pub fn to_bin(self) -> Result<Bin, UniErr> {
            Ok(Arc::new(bincode::serialize(&self)?))
        }
    }

    impl Deref for SubstanceList {
        type Target = Vec<Box<Substance>>;

        fn deref(&self) -> &Self::Target {
            &self.list
        }
    }

    impl DerefMut for SubstanceList {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.list
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
    pub struct ListPattern {
        pub primitive: SubstanceKind,
        pub range: NumRange,
    }

    impl ListPattern {
        pub fn is_match(&self, list: &SubstanceList) -> Result<(), UniErr> {
            /*
            for i in &list.list {
                if self.primitive != i.primitive_type() {
                    return Err(format!(
                        "Primitive List expected: {} found: {}",
                        self.primitive.to_string(),
                        i.primitive_type().to_string()
                    )
                    .into());
                }
            }

            Ok(())

             */
            unimplemented!()
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
    pub enum NumRange {
        MinMax { min: usize, max: usize },
        Exact(usize),
        Any,
    }
    pub type SubstanceTypePatternCtx = SubstanceTypePatternDef<PointCtx>;
    pub type SubstanceTypePatternVar = SubstanceTypePatternDef<PointVar>;

    #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
    pub enum SubstanceTypePatternDef<Pnt> {
        Empty,
        Primitive(SubstanceKind),
        List(ListPattern),
        Map(Box<MapPatternDef<Pnt>>),
    }

    impl ToResolved<SubstanceTypePatternDef<Point>> for SubstanceTypePatternDef<PointCtx> {
        fn to_resolved(self, env: &Env) -> Result<SubstanceTypePatternDef<Point>, UniErr> {
            match self {
                SubstanceTypePatternDef::Empty => Ok(SubstanceTypePatternDef::Empty),
                SubstanceTypePatternDef::Primitive(payload_type) => {
                    Ok(SubstanceTypePatternDef::Primitive(payload_type))
                }
                SubstanceTypePatternDef::List(list) => Ok(SubstanceTypePatternDef::List(list)),
                SubstanceTypePatternDef::Map(map) => {
                    Err("MapPatternCtx resolution not supported yet...".into())
                }
            }
        }
    }

    impl ToResolved<SubstanceTypePatternCtx> for SubstanceTypePatternVar {
        fn to_resolved(self, env: &Env) -> Result<SubstanceTypePatternCtx, UniErr> {
            match self {
                SubstanceTypePatternVar::Empty => Ok(SubstanceTypePatternCtx::Empty),
                SubstanceTypePatternVar::Primitive(payload_type) => {
                    Ok(SubstanceTypePatternCtx::Primitive(payload_type))
                }
                SubstanceTypePatternVar::List(list) => Ok(SubstanceTypePatternCtx::List(list)),
                SubstanceTypePatternVar::Map(map) => {
                    Err("MapPatternCtx resolution not supported yet...".into())
                }
            }
        }
    }

    impl<Pnt> SubstanceTypePatternDef<Pnt> {
        pub fn is_match(&self, payload: &Substance) -> Result<(), ()> {
            unimplemented!();
            /*
            match self {
                SubstanceTypePattern::Empty => {
                    if payload.payload_type() == SubstanceType::Empty {
                        Ok(())
                    } else {
                        Err(format!(
                            "Substance expected: Empty found: {}",
                            payload.payload_type().to_string()
                        )
                        .into())
                    }
                }
                SubstanceTypePattern::Primitive(expected) => {
                    if let Substance::Primitive(found) = payload {
                        if *expected == found.primitive_type() {
                            Ok(())
                        } else {
                            Err(format!(
                                "Substance Primitive expected: {} found: {}",
                                expected.to_string(),
                                found.primitive_type().to_string()
                            )
                            .into())
                        }
                    } else {
                        Err(format!(
                            "Substance expected: {} found: {}",
                            expected.to_string(),
                            payload.payload_type().to_string()
                        )
                        .into())
                    }
                }
                SubstanceTypePattern::List(expected) => {
                    if let Substance::List(found) = payload {
                        expected.is_match(found)
                    } else {
                        Err(format!(
                            "Substance expected: List found: {}",
                            payload.payload_type().to_string()
                        )
                        .into())
                    }
                }
                SubstanceTypePattern::Map(expected) => {
                    if let Substance::Map(found) = payload {
                        expected.is_match(found)
                    } else {
                        Err(format!(
                            "Substance expected: {} found: {}",
                            expected.to_string(),
                            payload.payload_type().to_string()
                        )
                        .into())
                    }
                }
            }

             */
        }
    }

    pub type SubstancePatternVar = SubstancePatternDef<PointVar>;
    pub type SubstancePatternCtx = SubstancePatternDef<PointCtx>;
    pub type SubstancePattern = SubstancePatternDef<Point>;

    #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
    pub struct SubstancePatternDef<Pnt> {
        pub structure: SubstanceTypePatternDef<Pnt>,
        pub format: Option<SubstanceFormat>,
        pub validator: Option<CallWithConfigDef<Pnt>>,
    }
    impl ToResolved<SubstancePatternCtx> for SubstancePatternVar {
        fn to_resolved(self, env: &Env) -> Result<SubstancePatternCtx, UniErr> {
            let mut errs = vec![];
            let structure = match self.structure.to_resolved(env) {
                Ok(structure) => Some(structure),
                Err(err) => {
                    errs.push(err);
                    None
                }
            };
            let validator = match self.validator {
                None => None,
                Some(validator) => match validator.to_resolved(env) {
                    Ok(validator) => Some(validator),
                    Err(err) => {
                        errs.push(err);
                        None
                    }
                },
            };

            if errs.is_empty() {
                Ok(SubstancePatternCtx {
                    structure: structure.expect("structure"),
                    validator: validator,
                    format: self.format,
                })
            } else {
                Err(ParseErrs::fold(errs).into())
            }
        }
    }

    impl ToResolved<SubstancePattern> for SubstancePatternCtx {
        fn to_resolved(self, resolver: &Env) -> Result<SubstancePattern, UniErr> {
            let mut errs = vec![];
            let structure = match self.structure.to_resolved(resolver) {
                Ok(structure) => Some(structure),
                Err(err) => {
                    errs.push(err);
                    None
                }
            };
            let validator = match self.validator {
                None => None,
                Some(validator) => match validator.to_resolved(resolver) {
                    Ok(validator) => Some(validator),
                    Err(err) => {
                        errs.push(err);
                        None
                    }
                },
            };

            if errs.is_empty() {
                Ok(SubstancePattern {
                    structure: structure.expect("structure"),
                    validator: validator,
                    format: self.format,
                })
            } else {
                Err(ParseErrs::fold(errs).into())
            }
        }
    }

    impl<Pnt> ValueMatcher<Substance> for SubstancePatternDef<Pnt> {
        fn is_match(&self, payload: &Substance) -> Result<(), ()> {
            self.structure.is_match(&payload)?;

            // more matching to come... not sure exactly how to match Format and Validation...
            Ok(())
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
    pub struct CallWithConfigDef<Pnt> {
        pub call: CallDef<Pnt>,
        pub config: Option<Pnt>,
    }

    pub type CallWithConfig = CallWithConfigDef<Point>;
    pub type CallWithConfigCtx = CallWithConfigDef<PointCtx>;
    pub type CallWithConfigVar = CallWithConfigDef<PointVar>;

    impl ToResolved<CallWithConfigCtx> for CallWithConfigVar {
        fn to_resolved(self, resolver: &Env) -> Result<CallWithConfigCtx, UniErr> {
            let mut errs = vec![];
            let call = match self.call.to_resolved(resolver) {
                Ok(call) => Some(call),
                Err(err) => {
                    errs.push(err);
                    None
                }
            };
            let config = match self.config {
                None => None,
                Some(config) => match config.to_resolved(resolver) {
                    Ok(config) => Some(config),
                    Err(err) => {
                        errs.push(err);
                        None
                    }
                },
            };

            if errs.is_empty() {
                Ok(CallWithConfigCtx {
                    call: call.expect("call"),
                    config,
                })
            } else {
                Err(ParseErrs::fold(errs).into())
            }
        }
    }
    impl ToResolved<CallWithConfig> for CallWithConfigCtx {
        fn to_resolved(self, resolver: &Env) -> Result<CallWithConfig, UniErr> {
            let mut errs = vec![];
            let call = match self.call.to_resolved(resolver) {
                Ok(call) => Some(call),
                Err(err) => {
                    errs.push(err);
                    None
                }
            };
            let config = match self.config {
                None => None,
                Some(config) => match config.to_resolved(resolver) {
                    Ok(config) => Some(config),
                    Err(err) => {
                        errs.push(err);
                        None
                    }
                },
            };

            if errs.is_empty() {
                Ok(CallWithConfig {
                    call: call.expect("call"),
                    config,
                })
            } else {
                Err(ParseErrs::fold(errs).into())
            }
        }
    }

    pub type Call = CallDef<Point>;
    pub type CallCtx = CallDef<PointCtx>;
    pub type CallVar = CallDef<PointVar>;

    impl ToResolved<Call> for CallCtx {
        fn to_resolved(self, env: &Env) -> Result<Call, UniErr> {
            Ok(Call {
                point: self.point.to_resolved(env)?,
                kind: self.kind,
            })
        }
    }

    impl ToResolved<CallCtx> for CallVar {
        fn to_resolved(self, env: &Env) -> Result<CallCtx, UniErr> {
            Ok(CallCtx {
                point: self.point.to_resolved(env)?,
                kind: self.kind,
            })
        }
    }

    impl ToResolved<Call> for CallVar {
        fn to_resolved(self, env: &Env) -> Result<Call, UniErr> {
            let call: CallCtx = self.to_resolved(env)?;
            call.to_resolved(env)
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
    pub struct CallDef<Pnt> {
        pub point: Pnt,
        pub kind: CallKind,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
    pub enum CallKind {
        Cmd(CmdCall),
        Hyp(HypCall),
        Ext(ExtCall),
        Http(HttpCall),
    }

    impl CallKind {
        /*
        pub fn core_with_body(self, body: Substance) -> Result<RequestCore, ExtErr> {
            Ok(match self {
                CallKind::Ext(msg) => RequestCore {
                    headers: Default::default(),
                    method: Method::Ext(ExtMethod::new(msg.method)?),
                    uri: Uri::from_str(msg.path.as_str())?,
                    body,
                },
                CallKind::Http(http) => RequestCore {
                    headers: Default::default(),
                    method: Method::Http(http.method),
                    uri: Uri::from_str(http.path.as_str())?,
                    body,
                },
            })
        }
         */
    }

    impl ToString for Call {
        fn to_string(&self) -> String {
            format!("{}^{}", self.point.to_string(), self.kind.to_string())
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
    pub struct ExtCall {
        pub path: Subst<Tw<String>>,
        pub method: ExtMethod,
    }

    impl ExtCall {
        pub fn new(method: ExtMethod, path: Subst<Tw<String>>) -> Self {
            Self { method, path }
        }
    }

    impl ToString for ExtCall {
        fn to_string(&self) -> String {
            format!("Ext<{}>{}", self.method.to_string(), self.path.to_string())
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
    pub struct CmdCall {
        pub path: Subst<Tw<String>>,
        pub method: CmdMethod,
    }

    impl CmdCall {
        pub fn new(method: CmdMethod, path: Subst<Tw<String>>) -> Self {
            Self { method, path }
        }
    }

    impl ToString for CmdCall {
        fn to_string(&self) -> String {
            format!("Cmd<{}>{}", self.method.to_string(), self.path.to_string())
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
    pub struct HypCall {
        pub path: Subst<Tw<String>>,
        pub method: HypMethod,
    }

    impl HypCall {
        pub fn new(method: HypMethod, path: Subst<Tw<String>>) -> Self {
            Self { method, path }
        }
    }

    impl ToString for HypCall {
        fn to_string(&self) -> String {
            format!("Hyp<{}>{}", self.method.to_string(), self.path.to_string())
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
    pub struct HttpCall {
        pub path: Subst<Tw<String>>,

        pub method: HttpMethod,
    }

    impl HttpCall {
        pub fn new(method: HttpMethod, path: Subst<Tw<String>>) -> Self {
            Self { method, path }
        }
    }

    impl ToString for HttpCall {
        fn to_string(&self) -> String {
            format!("Http<{}>{}", self.method.to_string(), self.path.to_string())
        }
    }

    /*
    impl FromStr for HttpMethod {
        type Err = Error;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            let input = s.to_uppercase();
            match input.as_str() {
                "GET" => Ok(HttpMethod::GET),
                "POST" => Ok(HttpMethod::POST),
                "PUT" => Ok(HttpMethod::Put),
                "DELETE" => Ok(HttpMethod::DELETE),
                "PATCH" => Ok(HttpMethod::Patch),
                "HEAD" => Ok(HttpMethod::Head),
                "CONNECT" => Ok(HttpMethod::Connect),
                "OPTIONS" => Ok(HttpMethod::Options),
                "TRACE"=>  Ok(HttpMethod::Trace),
                what => Err(format!("unrecognized http method.  found: {}", what ).into())
            }
        }
    }

     */

    impl ValueMatcher<HttpMethod> for HttpMethod {
        fn is_match(&self, found: &HttpMethod) -> Result<(), ()> {
            if *self == *found {
                Ok(())
            } else {
                Err(())
            }
        }
    }

    impl ToString for CallKind {
        fn to_string(&self) -> String {
            match self {
                CallKind::Ext(msg) => msg.to_string(),
                CallKind::Http(http) => http.to_string(),
                CallKind::Cmd(cmd) => cmd.to_string(),
                CallKind::Hyp(sys) => sys.to_string(),
            }
        }
    }

    #[derive(
        Debug,
        Clone,
        Eq,
        PartialEq,
        strum_macros::Display,
        strum_macros::EnumString,
        Serialize,
        Deserialize,
    )]
    pub enum SubstanceFormat {
        #[strum(serialize = "json")]
        Json,
        #[strum(serialize = "image")]
        Image,
    }

    pub type MapPattern = MapPatternDef<Point>;
    pub type MapPatternCtx = MapPatternDef<PointCtx>;
    pub type MapPatternVar = MapPatternDef<PointVar>;

    #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
    pub struct MapPatternDef<Pnt> {
        pub required: HashMap<String, ValuePattern<SubstancePatternDef<Pnt>>>,
        pub allowed: ValuePattern<SubstancePatternDef<Pnt>>,
    }

    impl<Pnt> Default for MapPatternDef<Pnt> {
        fn default() -> Self {
            MapPatternDef {
                required: Default::default(),
                allowed: ValuePattern::Any,
            }
        }
    }

    impl<Pnt> ToString for MapPatternDef<Pnt> {
        fn to_string(&self) -> String {
            "Map?".to_string()
        }
    }

    impl<Pnt> MapPatternDef<Pnt> {
        pub fn new(
            required: HashMap<String, ValuePattern<SubstancePatternDef<Pnt>>>,
            allowed: ValuePattern<SubstancePatternDef<Pnt>>,
        ) -> Self {
            MapPatternDef { required, allowed }
        }

        pub fn empty() -> Self {
            Self {
                required: HashMap::new(),
                allowed: ValuePattern::None,
            }
        }

        pub fn any() -> Self {
            Self {
                required: HashMap::new(),
                allowed: ValuePattern::Any,
            }
        }

        pub fn is_match(&self, map: &SubstanceMap) -> Result<(), ()> {
            // if Any keys are allowed then skip
            for (key, payload) in &map.map {
                if !self.required.contains_key(key) {
                    match &self.allowed {
                        ValuePattern::Any => {}
                        ValuePattern::None => {
                            return Err(());
                        }
                        ValuePattern::Pattern(pattern) => {
                            pattern.is_match(payload)?;
                        }
                    }
                }
            }

            // now make sure all required are present and meet constraints
            for (key, constraint) in &self.required {
                if !map.contains_key(key) {
                    return Err(());
                }
                constraint.is_match(
                    &map.get(key)
                        .expect("expected map element after testing for it"),
                )?;
            }

            Ok(())
        }
    }

    /*
    impl<FromResourceType,FromKind,FromSubstance,FromTksPattern, ToResourceType,ToKind,ToSubstance,ToTksPattern> ConvertFrom<Valuepattern<SubstancePattern<FromResourceType,FromKind,FromSubstance,FromTksPattern>>>
    for ValuePattern<SubstancePattern<FromResourceType,FromKind,FromSubstance,FromTksPattern>>
        where
            FromKind: TryInto<ToKind, Error = Error> + Clone,
            FromTksPattern: TryInto<ToTksPattern, Error = Error> + Clone,
            FromSubstance: TryInto<ToSubstance, Error = Error> + Clone,
            FromResourceType: TryInto<ToResourceType, Error = Error> + Clone,
            ToKind: Clone,

    {

        fn convert_from(
            a: HashMap<String, ValuePattern<SubstancePattern<FromKind>>>
        ) -> Result<Self, Error>
            where
                Self: Sized,
        {
            let mut rtn = HashMap::new();
            for (k,v) in a {
                rtn.insert( k, ConvertFrom::convert_from(v)?);
            }
            Ok(rtn)
        }
    }

     */

    /*
    impl <KEY,ADDRESS,IDENTIFIER,KIND> ValuePattern<Substance<KEY,ADDRESS,IDENTIFIER,KIND>> for SubstanceType<KEY,ADDRESS,IDENTIFIER,KIND> {
        fn is_match(&self, payload: &Substance<KEY,ADDRESS,IDENTIFIER,KIND>) -> Result<(), Error> {
            match **self {
                SubstanceType::Empty => {
                    if let Substance::Empty = payload {
                        Ok(())
                    } else {
                        Err(format!("Substance expected: '{}' found: Empty",self.to_string()).into())
                    }
                }
                SubstanceType::Primitive(expected) => {
                    if let Substance::Primitive(found)= payload {
                        expected.is_match(found)
                    } else {
                        Err(format!("Substance expected: '{}' found: '{}'",self.to_string(), payload.to_string()).into())
                    }
                }
                SubstanceType::List(expected) => {
                    if let Substance::List(found)= payload {
                        expected.is_match(&found.primitive_type )
                    } else {
                        Err(format!("Substance expected: '{}' found: '{}'",self.to_string(), payload.to_string()).into())
                    }
                }
                SubstanceType::Map(expected) => {
                    if let Substance::Map(found)= payload {
                        expected.is_match(&found.primitive_type )
                    } else {
                        Err(format!("Substance expected: '{}' found: '{}'",self.to_string(), payload.to_string()).into())
                    }
                }
            }
        }
    }

     */

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SubstanceRef<PAYLOAD_CLAIM, PAYLOAD_PATTERN> {
        pub claim: PAYLOAD_CLAIM,
        pub pattern: PAYLOAD_PATTERN,
    }

    /*
    impl<FromSubstanceClaim, FromSubstancePattern> SubstanceRef<FromSubstanceClaim, FromSubstancePattern> {
        pub fn convert<ToSubstanceClaim, ToSubstancePattern>(
            self,
        ) -> Result<SubstanceRef<ToSubstanceClaim, ToSubstancePattern>, Error>
        where
            ToSubstanceClaim: TryFrom<FromSubstanceClaim, Error = Error>,
            ToSubstancePattern: TryFrom<FromSubstancePattern, Error = Error>,
        {
            Ok(Self {
                claim: self.claim.try_into()?,
                pattern: self.pattern.try_into()?,
            })
        }
    }

     */

    /*    #[derive(Debug, Clone, Serialize, Deserialize)]
       pub enum SubstanceDelivery<PAYLOAD, PAYLOAD_REF> {
           Substance(PAYLOAD),
           Ref(PAYLOAD_REF),
       }

    */

    /*
    impl<FromSubstance, FromSubstanceRef> SubstanceDelivery<FromSubstance, FromSubstanceRef> {
        pub fn convert<ToSubstance, ToSubstanceRef>(
            self,
        ) -> Result<SubstanceDelivery<ToSubstance, ToSubstanceRef>, Error>
        where
            ToSubstance: TryFrom<FromSubstance,Error=Error>,
            ToSubstanceRef: TryFrom<FromSubstanceRef,Error=Error>,
        {
            match self {
                SubstanceDelivery::Substance(payload) => Ok(payload.try_into()?),
                SubstanceDelivery::Ref(payload_ref) => {
                    Ok(payload_ref.try_into()?)
                }
            }
        }
    }
     */

    #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
    pub struct MultipartForm {
        data: String,
    }

    impl TryInto<HashMap<String, String>> for MultipartForm {
        type Error = UniErr;

        fn try_into(self) -> Result<HashMap<String, String>, Self::Error> {
            let map: HashMap<String, String> = serde_urlencoded::from_str(&self.data)?;
            Ok(map)
        }
    }

    impl ToRequestCore for MultipartForm {
        type Method = HttpMethod;

        fn to_request_core(self) -> DirectedCore {
            let mut headers = HeaderMap::new();

            headers.insert(
                CONTENT_TYPE,
                HeaderValue::from_static("application/x-www-form-urlencoded"),
            );

            DirectedCore {
                headers,
                method: HttpMethod::Post.into(),
                uri: Default::default(),
                body: Substance::MultipartForm(self),
            }
        }
    }

    impl MultipartForm {
        pub fn data(&self) -> &str {
            self.data.as_str()
        }
    }

    impl Deref for MultipartForm {
        type Target = str;

        fn deref(&self) -> &Self::Target {
            self.data()
        }
    }

    impl ToString for MultipartForm {
        fn to_string(&self) -> String {
            self.data.clone()
        }
    }

    pub struct MultipartFormBuilder {
        map: HashMap<String, String>,
    }

    impl MultipartFormBuilder {
        pub fn new() -> Self {
            Self {
                map: HashMap::new(),
            }
        }

        pub fn put<S: ToString>(&mut self, key: S, value: S) {
            self.insert(key.to_string(), value.to_string());
        }

        pub fn get<S: ToString>(&self, key: S) -> Option<&String> {
            self.map.get(&key.to_string())
        }
    }

    impl Deref for MultipartFormBuilder {
        type Target = HashMap<String, String>;

        fn deref(&self) -> &Self::Target {
            &self.map
        }
    }

    impl DerefMut for MultipartFormBuilder {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.map
        }
    }

    impl MultipartFormBuilder {
        pub fn build(self) -> Result<MultipartForm, UniErr> {
            let data = serde_urlencoded::to_string(&self.map)?;
            Ok(MultipartForm { data })
        }
    }
}

pub type Bin = Arc<Vec<u8>>;
