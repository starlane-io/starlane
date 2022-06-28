use core::str::FromStr;
use nom::combinator::all_consuming;
use crate::error::MsgErr;
use crate::version::v0_0_1::command::request::create::{Create, CreateCtx, CreateVar, Strategy};
use crate::version::v0_0_1::command::request::delete::{Delete, DeleteCtx, DeleteVar};
use crate::version::v0_0_1::command::request::get::{Get, GetCtx, GetVar};
use crate::version::v0_0_1::command::request::select::{Select, SelectCtx, SelectVar};
use crate::version::v0_0_1::command::request::set::{Set, SetCtx, SetVar};
use crate::version::v0_0_1::parse::{command_line, Env};
use crate::version::v0_0_1::parse::error::result;
use cosmic_nom::new_span;
use crate::version::v0_0_1::util::ToResolved;
use serde::{Deserialize, Serialize};
use crate::version::v0_0_1::command::request::read::{Read, ReadCtx, ReadVar};
use crate::version::v0_0_1::command::request::update::{Update, UpdateCtx, UpdateVar};
use crate::version::v0_0_1::wave::CmdMethod;
use cosmic_macros_primitive::Autobox;

pub mod command {
    use serde::{Deserialize, Serialize};



    pub mod common {
        use std::collections::HashMap;
        use std::convert::{TryFrom, TryInto};
        use std::ops::{Deref, DerefMut};

        use serde::{Deserialize, Serialize};

        use crate::error::MsgErr;
        use crate::version::v0_0_1::id::id::Variable;
        use crate::version::v0_0_1::parse::model::Var;
        use crate::version::v0_0_1::substance::substance::{Substance, SubstanceMap};


        #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, strum_macros::Display)]
        pub enum StateSrcVar {
            None,
            FileRef(String),
            Var(Variable)
        }


        #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, strum_macros::Display)]
        pub enum StateSrc {
            None,
            Substance(Box<Substance>),
        }

        #[derive(Debug, Clone, Serialize, Deserialize,Eq,PartialEq)]
        pub enum PropertyMod {
            Set {
                key: String,
                value: String,
                lock: bool,
            },
            UnSet(String),
        }

        impl PropertyMod {
            pub fn set_or<E>(&self, err: E) -> Result<String, E> {
                match self {
                    Self::Set { key, value, lock } => Ok(value.clone()),
                    Self::UnSet(_) => Err(err),
                }
            }

            pub fn opt(&self) -> Option<String> {
                match self {
                    Self::Set { key, value, lock } => Some(value.clone()),
                    Self::UnSet(_) => None,
                }
            }
        }

        #[derive(Debug, Clone, Serialize, Deserialize,Eq,PartialEq)]
        pub struct SetProperties {
            pub map: HashMap<String, PropertyMod>,
        }

        impl Default for SetProperties {
            fn default() -> Self {
                Self {
                    map: Default::default(),
                }
            }
        }

        impl SetProperties {
            pub fn new() -> Self {
                Self {
                    map: HashMap::new(),
                }
            }

            pub fn append(&mut self, properties: SetProperties) {
                for (_, property) in properties.map.into_iter() {
                    self.push(property);
                }
            }

            pub fn push(&mut self, property: PropertyMod) {
                match &property {
                    PropertyMod::Set { key, value, lock } => {
                        self.map.insert(key.clone(), property);
                    }
                    PropertyMod::UnSet(key) => {
                        self.map.insert(key.clone(), property);
                    }
                }
            }
        }

        impl Deref for SetProperties {
            type Target = HashMap<String, PropertyMod>;

            fn deref(&self) -> &Self::Target {
                &self.map
            }
        }

        #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, strum_macros::Display)]
        pub enum SetLabel {
            Set(String),
            SetValue { key: String, value: String },
            Unset(String),
        }

        #[derive(Debug, Clone, Serialize, Deserialize,Eq,PartialEq)]
        pub struct SetRegistry {
            pub labels: Vec<SetLabel>,
        }

        impl Deref for SetRegistry {
            type Target = Vec<SetLabel>;

            fn deref(&self) -> &Self::Target {
                &self.labels
            }
        }

        impl DerefMut for SetRegistry {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.labels
            }
        }

        impl Default for SetRegistry {
            fn default() -> Self {
                Self {
                    labels: Default::default(),
                }
            }
        }
    }
}

pub mod request {
    use crate::error::{MsgErr, StatusErr};
    use crate::version::v0_0_1::bin::Bin;
    use crate::version::v0_0_1::command::request::create::Create;
    use crate::version::v0_0_1::command::request::get::Get;
    use crate::version::v0_0_1::command::request::select::Select;
    use crate::version::v0_0_1::command::request::set::Set;
    use crate::version::v0_0_1::command::request::update::Update;
    use crate::version::v0_0_1::wave::RespCore;
    use crate::version::v0_0_1::fail;
    use crate::version::v0_0_1::fail::{BadRequest, Fail, NotFound};
    use crate::version::v0_0_1::id::id::{KindParts, BaseKind, Meta, Point};
    use crate::version::v0_0_1::substance::substance::{Errors, Substance, };
    use crate::version::v0_0_1::selector::selector::KindSelector;
    use crate::version::v0_0_1::util::{ValueMatcher, ValuePattern};
    use http::status::InvalidStatusCode;
    use http::{HeaderMap, Request, StatusCode, Uri};
    use serde::{Deserialize, Serialize};
    use crate::version::v0_0_1::wave::MethodKind;
    use crate::version::v0_0_1::http::HttpMethod;
    use crate::version::v0_0_1::msg::MsgMethod;

    #[derive(Debug, Clone,Serialize,Deserialize)]
    pub enum Rc {
        Create(Create),
        Select(Select),
        Update(Update),
        Get(Get),
        Set(Set),
    }

    impl PartialEq<Self> for Rc {
        fn eq(&self, other: &Self) -> bool {
            self.get_type() == other.get_type()
        }
    }

    impl Eq for Rc {

    }

    impl Rc {
        pub fn get_type(&self) -> RcCommandType {
            match self {
                Rc::Create(_) => RcCommandType::Create,
                Rc::Select(_) => RcCommandType::Select,
                Rc::Update(_) => RcCommandType::Update,
                Rc::Get(_) => RcCommandType::Get,
                Rc::Set(_) => RcCommandType::Set,
            }
        }
    }

    /*
    impl Rc {
        pub fn command_handler(&self, request_to: &Address) -> Result<Address,Error> {
            match self {
                Rc::Create(create) => { Ok(create.template.point.parent.clone()) }
                Rc::Select(select) => { Ok(select.pattern.query_root()) }
                Rc::Update(_) => {request_to.clone()}
                Rc::Query(_) => { request_to.clone()}
                Rc::GET(_) => {request_to.parent().as_ref().ok_or("expected parent for get request").clone()}
                Rc::Set(_) => {request_to.parent().as_ref().ok_or("expected parent for set request").clone()}
            }
        }
    }

     */

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
    pub enum RcCommandType {
        Create,
        Select,
        Update,
        Query,
        Get,
        Set,
    }

    impl ValueMatcher<Rc> for Rc {
        fn is_match(&self, x: &Rc) -> Result<(), ()> {
            if self.get_type() == x.get_type() {
                Ok(())
            } else {
                Err(())
            }
        }
    }

    impl ToString for Rc {
        fn to_string(&self) -> String {
            format!("Rc<{}>", self.get_type().to_string())
        }
    }

    pub mod set {
        use crate::error::MsgErr;
        use crate::version::v0_0_1::command::command::common::SetProperties;
        use crate::version::v0_0_1::id::id::{Point, PointCtx, PointVar};
        use crate::version::v0_0_1::parse::Env;
        use serde::{Deserialize, Serialize};
        use crate::version::v0_0_1::util::ToResolved;

        pub type Set = SetDef<Point>;
        pub type SetCtx = SetDef<PointCtx>;
        pub type SetVar = SetDef<PointVar>;

        #[derive(Debug, Clone, Serialize, Deserialize,Eq,PartialEq)]
        pub struct SetDef<Pnt> {
            pub point: Pnt,
            pub properties: SetProperties,
        }

        impl ToResolved<Set> for SetVar {
            fn to_resolved(self, env: &Env) -> Result<Set, MsgErr> {
                let set: SetCtx = self.to_resolved(env)?;
                set.to_resolved(env)
            }
        }

        impl ToResolved<SetCtx> for SetVar {
            fn to_resolved(self, env: &Env) -> Result<SetCtx, MsgErr> {
                Ok(SetCtx {
                    point: self.point.to_resolved(env)?,
                    properties: self.properties,
                })
            }
        }

        impl ToResolved<Set> for SetCtx {
            fn to_resolved(self, env: &Env) -> Result<Set, MsgErr> {
                Ok(Set {
                    point: self.point.to_resolved(env)?,
                    properties: self.properties,
                })
            }
        }
    }

    pub mod get {
        use crate::error::MsgErr;
        use crate::version::v0_0_1::command::command::common::SetProperties;
        use crate::version::v0_0_1::id::id::{Point, PointCtx, PointVar};
        use crate::version::v0_0_1::parse::Env;
        use serde::{Deserialize, Serialize};
        use crate::version::v0_0_1::util::ToResolved;

        pub type Get = GetDef<Point>;
        pub type GetCtx = GetDef<PointCtx>;
        pub type GetVar = GetDef<PointVar>;

        #[derive(Debug, Clone, Serialize, Deserialize,Eq,PartialEq)]
        pub struct GetDef<Pnt> {
            pub point: Pnt,
            pub op: GetOp,
        }

        impl ToResolved<Get> for GetVar {
            fn to_resolved(self, env: &Env) -> Result<Get, MsgErr> {
                let set: GetCtx = self.to_resolved(env)?;
                set.to_resolved(env)
            }
        }

        impl ToResolved<GetCtx> for GetVar {
            fn to_resolved(self, env: &Env) -> Result<GetCtx, MsgErr> {
                Ok(GetCtx {
                    point: self.point.to_resolved(env)?,
                    op: self.op,
                })
            }
        }

        impl ToResolved<Get> for GetCtx {
            fn to_resolved(self, env: &Env) -> Result<Get, MsgErr> {
                Ok(Get {
                    point: self.point.to_resolved(env)?,
                    op: self.op,
                })
            }
        }

        #[derive(Debug, Clone, Serialize, Deserialize,Eq,PartialEq)]
        pub enum GetOp {
            State,
            Properties(Vec<String>),
        }
    }

    pub mod create {
        use std::convert::TryInto;
        use std::sync::atomic::{AtomicU128, Ordering};

        use serde::{Deserialize, Serialize};

        use crate::error::{MsgErr, ParseErrs};
        use crate::version::v0_0_1::bin::Bin;
        use crate::version::v0_0_1::command::Command;
        use crate::version::v0_0_1::command::command::common::{SetProperties, SetRegistry, StateSrc, StateSrcVar};
        use crate::version::v0_0_1::id::id::{KindParts, BaseKind, HostKey, Point, PointCtx, PointSeg, PointVar, ToPort};
        use crate::version::v0_0_1::wave::{CmdMethod, ReqProto, ReqCore, SysMethod};
        use crate::version::v0_0_1::msg::MsgMethod;
        use crate::version::v0_0_1::parse::{CamelCase, Env, ResolverErr};
        use crate::version::v0_0_1::substance::substance::Substance;
        use crate::version::v0_0_1::selector::selector::SpecificSelector;
        use crate::version::v0_0_1::util::{ConvertFrom, ToResolved};

        pub enum PointTemplateSeg {
            ExactSeg(PointSeg),
            Wildcard(String),
        }

        impl PointTemplateSeg {
            pub fn is_wildcard(&self) -> bool {
                match self {
                    PointTemplateSeg::ExactSeg(_) => false,
                    PointTemplateSeg::Wildcard(_) => true,
                }
            }
        }

        pub type Template = TemplateDef<PointTemplate>;
        pub type TemplateCtx = TemplateDef<PointTemplateCtx>;
        pub type TemplateVar = TemplateDef<PointTemplateVar>;

        #[derive(Debug, Clone, Serialize, Deserialize,Eq,PartialEq)]
        pub struct TemplateDef<Pnt> {
            pub point: Pnt,
            pub kind: KindTemplate,
        }

        impl ToResolved<Template> for TemplateVar {
            fn to_resolved(self, env: &Env) -> Result<Template, MsgErr> {
                let template: TemplateCtx = self.to_resolved(env)?;
                template.to_resolved(env)
            }
        }

        impl ToResolved<TemplateCtx> for TemplateVar {
            fn to_resolved(self, env: &Env) -> Result<TemplateCtx, MsgErr> {
                let point : PointTemplateCtx = self.point.to_resolved(env)?;

                let template = TemplateCtx {
                    point,
                    kind: KindTemplate {
                        base: BaseKind::Bundle,
                        sub: None,
                        specific: None,
                    },
                };
                Ok(template)
            }
        }
        impl ToResolved<Template> for TemplateCtx {
            fn to_resolved(self, env: &Env) -> Result<Template, MsgErr> {
                let point = self.point.to_resolved(env)?;

                let template = Template {
                    point,
                    kind: KindTemplate {
                        base: BaseKind::Bundle,
                        sub: None,
                        specific: None,
                    },
                };
                Ok(template)
            }
        }

        impl Template {
            pub fn new(point: PointTemplate, kind: KindTemplate) -> Self {
                Self { point, kind }
            }
        }

        #[derive(Debug, Clone, Serialize, Deserialize,Eq,PartialEq)]
        pub struct KindTemplate {
            pub base: BaseKind,
            pub sub: Option<CamelCase>,
            pub specific: Option<SpecificSelector>,
        }

        impl TryInto<KindParts> for KindTemplate {
            type Error = MsgErr;

            fn try_into(self) -> Result<KindParts, Self::Error> {
                if self.specific.is_some() {
                    return Err("cannot create a ResourceKind from a specific pattern when using KindTemplate".into());
                }
                Ok(KindParts {
                    base: self.base,
                    sub: self.sub,
                    specific: None,
                })
            }
        }

        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub enum Require {
            File(String),
            Auth(String),
        }

        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub enum Fulfillment {
            File { name: String, content: Bin },
            Complete,
        }

        pub type Create = CreateDef<Point,StateSrc>;
        pub type CreateVar = CreateDef<PointVar,StateSrcVar>;
        pub type CreateCtx = CreateDef<PointCtx,StateSrc>;

        impl ToResolved<Create> for CreateVar {
            fn to_resolved(self, env: &Env) -> Result<Create, MsgErr> {
                let create: CreateCtx = self.to_resolved(env)?;
                create.to_resolved(env)
            }
        }


        impl ToResolved<CreateCtx> for CreateVar {
            fn to_resolved(self, env: &Env) -> Result<CreateCtx, MsgErr> {
                let template = self.template.to_resolved(env)?;
                let state = match &self.state {
                    StateSrcVar::None => StateSrc::None,
                    StateSrcVar::FileRef(name) => StateSrc::Substance(Box::new(Substance::Bin(env.file(name).map_err(|e|{ match e{
                        ResolverErr::NotAvailable => MsgErr::from_500("files are not available in this context"),
                        ResolverErr::NotFound => MsgErr::from_500(format!("cannot find file '{}'",name))
                    }})?.content))),
                    StateSrcVar::Var(var) => {
                        let val = env.val(var.name.as_str() ).map_err(|e|match e{
                            ResolverErr::NotAvailable => MsgErr::from_500("variable are not available in this context"),
                            ResolverErr::NotFound => MsgErr::from_500(format!("cannot find variable '{}'", var.name ))
                        })?;
                        StateSrc::Substance(Box::new(Substance::Bin(env.file(val.clone()).map_err(|e|{ match e{
                            ResolverErr::NotAvailable => MsgErr::from_500("files are not available in this context"),
                            ResolverErr::NotFound => MsgErr::from_500(format!("cannot find file '{}'",val))
                        }})?.content)))
                    }
                };
                Ok(CreateCtx {
                    template,
                    properties: self.properties,
                    strategy: self.strategy,
                    registry: self.registry,
                    state,
                })
            }
        }

        impl ToResolved<Create> for CreateCtx {
            fn to_resolved(self, env: &Env) -> Result<Create, MsgErr> {
                let template = self.template.to_resolved(env)?;
                Ok(Create {
                    template,
                    properties: self.properties,
                    strategy: self.strategy,
                    registry: self.registry,
                    state: self.state,
                })
            }
        }




        #[derive(Debug, Clone,Serialize,Deserialize,Eq,PartialEq)]
        pub struct CreateDef<Pnt,StateSrc> {
            pub template: TemplateDef<PointTemplateDef<Pnt>>,
            pub properties: SetProperties,
            pub strategy: Strategy,
            pub registry: SetRegistry,
            pub state: StateSrc,
        }

        impl Create {
            pub fn fulfillment(mut self, bin: Bin) -> Create {
                Create {
                    template: self.template,
                    state: StateSrc::Substance(Box::new(Substance::Bin(bin))),
                    properties: self.properties,
                    strategy: self.strategy,
                    registry: self.registry,
                }
            }
        }

        impl Into<ReqCore> for Create {
            fn into(self) -> ReqCore {
                let mut request = ReqCore::msg(MsgMethod::new("Command").unwrap() );
                request.body = Substance::Command(Box::new(Command::Create(self)));
                request
            }
        }


        impl Into<ReqProto> for Create {
            fn into(self) -> ReqProto {
                let mut request = ReqProto::sys(Point::global_executor().to_port(), SysMethod::Command);
                request.body(Substance::Command(Box::new(Command::Create(self))));
                request
            }
        }


        #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
        pub enum Strategy {
            Commit,
            Ensure,
        }

        pub trait PointFactory: Send+Sync {
            fn create(&mut self) -> Result<Point,MsgErr>;
        }

        pub struct PointFactoryU128 {
            parent: Point,
            prefix: String,
            atomic: AtomicU128
        }

        impl PointFactoryU128 {
            pub fn new( parent: Point, prefix: String ) -> Self {
                Self {
                    parent,
                    prefix,
                    atomic: AtomicU128::new(0u128 )
                }
            }
        }


        impl PointFactory for PointFactoryU128 {
            fn create(&mut self) -> Result<Point, MsgErr> {
                let index = self.atomic.fetch_add( 1u128,Ordering::Relaxed );
                self.parent.push( format!("{}{}", self.prefix, index))
            }
        }


        pub type PointTemplate = PointTemplateDef<Point>;
        pub type PointTemplateCtx = PointTemplateDef<PointCtx>;
        pub type PointTemplateVar = PointTemplateDef<PointVar>;

        #[derive(Debug, Clone, Serialize, Deserialize,Eq,PartialEq)]
        pub struct PointTemplateDef<Pnt> {
            pub parent: Pnt,
            pub child_segment_template: PointSegTemplate,
        }

        impl ToResolved<PointTemplateCtx> for PointTemplateVar {
            fn to_resolved(self, env: &Env) -> Result<PointTemplateCtx, MsgErr> {
                let parent = self.parent.to_resolved(env)?;
                Ok(PointTemplateCtx {
                    parent,
                    child_segment_template: self.child_segment_template
                })
            }
        }

        impl ToResolved<PointTemplate> for PointTemplateCtx{
            fn to_resolved(self, env: &Env) -> Result<PointTemplate, MsgErr> {
                let parent = self.parent.to_resolved(env)?;
                Ok(PointTemplate{
                    parent,
                    child_segment_template: self.child_segment_template
                })
            }
        }

        impl ToResolved<PointTemplate> for PointTemplateVar {
            fn to_resolved(self, env: &Env) -> Result<PointTemplate, MsgErr> {
                let ctx: PointTemplateCtx = self.to_resolved(env)?;
                ctx.to_resolved(env)
            }
        }



        #[derive(Debug, Clone, strum_macros::Display, Serialize, Deserialize,Eq,PartialEq)]
        pub enum PointSegTemplate {
            Exact(String),
            Pattern(String), // must have a '%'
        }
    }

    pub mod select {
        use std::collections::{HashMap, HashSet};
        use std::convert::{TryFrom, TryInto};
        use std::marker::PhantomData;

        use serde::{Deserialize, Serialize};

        use crate::error::MsgErr;
        use crate::version::v0_0_1::fail::{BadCoercion, Fail};
        use crate::version::v0_0_1::id::id::Point;
        use crate::version::v0_0_1::parse::Env;
        use crate::version::v0_0_1::particle::particle::Stub;
        use crate::version::v0_0_1::substance::substance::{
            MapPattern, Substance, SubstanceList,
        };
        use crate::version::v0_0_1::selector::selector::{Hop, HopCtx, HopVar, PointHierarchy, PointSelector, PointSelectorDef};
        use crate::version::v0_0_1::util::{ConvertFrom, ToResolved};

        #[derive(Debug, Clone, Serialize, Deserialize,Eq,PartialEq)]
        pub enum SelectIntoSubstance {
            Stubs,
            Points,
        }

        impl SelectIntoSubstance {
            pub fn to_primitive(&self, stubs: Vec<Stub>) -> Result<SubstanceList, MsgErr> {
                match self {
                    SelectIntoSubstance::Stubs => {
                        let stubs: Vec<Box<Substance>> = stubs
                            .into_iter()
                            .map(|stub| Box::new(Substance::Stub(stub)))
                            .collect();
                        let stubs = SubstanceList { list: stubs };
                        Ok(stubs)
                    }
                    SelectIntoSubstance::Points => {
                        let pointes: Vec<Box<Substance>> = stubs
                            .into_iter()
                            .map(|stub| Box::new(Substance::Point(stub.point)))
                            .collect();
                        let stubs = SubstanceList { list: pointes };
                        Ok(stubs)
                    }
                }
            }
        }

        pub type Select = SelectDef<Hop>;
        pub type SelectCtx = SelectDef<Hop>;
        pub type SelectVar = SelectDef<Hop>;

        impl ToResolved<Select> for Select{
            fn to_resolved(self, env: &Env) -> Result<Select, MsgErr> {
                Ok(self)
            }
        }

        #[derive(Debug, Clone,Serialize,Deserialize,Eq,PartialEq)]
        pub struct SelectDef<Hop> {
            pub pattern: PointSelectorDef<Hop>,
            pub properties: PropertiesPattern,
            pub into_payload: SelectIntoSubstance,
            pub kind: SelectKind,
        }

        #[derive(Debug, Clone, Serialize, Deserialize,Eq,PartialEq)]
        pub enum SelectKind {
            Initial,
            SubSelect {
                point: Point,
                hops: Vec<Hop>,
                hierarchy: PointHierarchy,
            },
        }

        impl Select {
            pub fn sub_select(
                self,
                point: Point,
                hops: Vec<Hop>,
                hierarchy: PointHierarchy,
            ) -> SubSelect {
                SubSelect {
                    point,
                    pattern: self.pattern,
                    properties: self.properties,
                    into_payload: self.into_payload,
                    hops,
                    hierarchy,
                }
            }
        }

        impl TryInto<SubSelect> for Select {
            type Error = MsgErr;

            fn try_into(self) -> Result<SubSelect, Self::Error> {
                if let SelectKind::SubSelect {
                    point,
                    hops,
                    hierarchy,
                } = self.kind
                {
                    Ok(SubSelect {
                        point,
                        pattern: self.pattern,
                        properties: self.properties,
                        into_payload: self.into_payload,
                        hops: hops,
                        hierarchy,
                    })
                } else {
                    Err("Not of kind SubSelector".into())
                }
            }
        }

        #[derive(Debug, Clone )]
        pub struct SubSelect {
            pub point: Point,
            pub pattern: PointSelector,
            pub properties: PropertiesPattern,
            pub into_payload: SelectIntoSubstance,
            pub hops: Vec<Hop>,
            pub hierarchy: PointHierarchy,
        }

        impl Into<Select> for SubSelect {
            fn into(self) -> Select {
                Select {
                    pattern: self.pattern,
                    properties: self.properties,
                    into_payload: self.into_payload,
                    kind: SelectKind::SubSelect {
                        point: self.point,
                        hops: self.hops,
                        hierarchy: self.hierarchy,
                    },
                }
            }
        }

        impl SubSelect {
            pub fn sub_select(
                &self,
                point: Point,
                hops: Vec<Hop>,
                hierarchy: PointHierarchy,
            ) -> SubSelect {
                SubSelect {
                    point,
                    pattern: self.pattern.clone(),
                    properties: self.properties.clone(),
                    into_payload: self.into_payload.clone(),
                    hops,
                    hierarchy,
                }
            }
        }

        impl Select {
            pub fn new(pattern: PointSelector) -> Self {
                Self {
                    pattern,
                    properties: Default::default(),
                    into_payload: SelectIntoSubstance::Stubs,
                    kind: SelectKind::Initial,
                }
            }
        }

        pub type PropertiesPattern = MapPattern;
    }

    pub mod delete {
        use crate::error::MsgErr;
        use crate::version::v0_0_1::command::request::select::{PropertiesPattern, Select, SelectIntoSubstance};
        use crate::version::v0_0_1::parse::Env;
        use crate::version::v0_0_1::selector::selector::{Hop, PointSelectorDef};
        use crate::version::v0_0_1::util::ToResolved;
        use serde::{Deserialize, Serialize};

        pub type Delete = DeleteDef<Hop>;
        pub type DeleteCtx = DeleteDef<Hop>;
        pub type DeleteVar = DeleteDef<Hop>;

        impl ToResolved<Delete> for Delete{
            fn to_resolved(self, env: &Env) -> Result<Delete, MsgErr> {
                Ok(self)
            }
        }

        #[derive(Debug, Clone,Serialize,Deserialize,Eq,PartialEq)]
        pub struct DeleteDef<Hop> {
            pub selector: PointSelectorDef<Hop>,
        }

        impl Into<Select> for Delete {
            fn into(self) -> Select {
                let mut select = Select::new(self.selector );
                select.into_payload = SelectIntoSubstance::Points;
                select
            }
        }

    }

    pub mod update {
        use std::convert::TryInto;

        use serde::{Deserialize, Serialize};

        use crate::error::MsgErr;
        use crate::version::v0_0_1::command::command::common::SetProperties;
        use crate::version::v0_0_1::id::id::{Point, PointCtx, PointVar};
        use crate::version::v0_0_1::parse::Env;
        use crate::version::v0_0_1::substance::substance::Substance;
        use crate::version::v0_0_1::util::ToResolved;

        pub type Update = UpdateDef<Point>;
        pub type UpdateCtx = UpdateDef<PointCtx>;
        pub type UpdateVar = UpdateDef<PointVar>;

        #[derive(Debug, Clone, Serialize, Deserialize,Eq,PartialEq)]
        pub struct UpdateDef<Pnt> {
            pub point: Pnt,
            pub payload: Substance,
        }

        impl ToResolved<UpdateCtx> for UpdateVar {
            fn to_resolved(self, env: &Env) -> Result<UpdateCtx, MsgErr> {
                Ok(UpdateCtx {
                    point: self.point.to_resolved(env)?,
                    payload: self.payload
                })
            }
        }

        impl ToResolved<Update> for UpdateCtx {
            fn to_resolved(self, env: &Env) -> Result<Update, MsgErr> {
                Ok(Update {
                    point: self.point.to_resolved(env)?,
                    payload: self.payload
                })
            }
        }


    }

    pub mod read {
        use crate::error::MsgErr;
        use crate::version::v0_0_1::id::id::{Point, PointCtx, PointVar};
        use crate::version::v0_0_1::parse::Env;
        use crate::version::v0_0_1::substance::substance::Substance;
        use crate::version::v0_0_1::util::ToResolved;
        use serde::{Serialize,Deserialize};

        pub type Read = ReadDef<Point>;
        pub type ReadCtx = ReadDef<PointCtx>;
        pub type ReadVar = ReadDef<PointVar>;

        #[derive(Debug, Clone, Serialize, Deserialize,Eq,PartialEq)]
        pub struct ReadDef<Pnt> {
            pub point: Pnt,
            pub payload: Substance,
        }

        impl ToResolved<ReadCtx> for ReadVar {
            fn to_resolved(self, env: &Env) -> Result<ReadCtx, MsgErr> {
                Ok(ReadCtx {
                    point: self.point.to_resolved(env)?,
                    payload: self.payload
                })
            }
        }

        impl ToResolved<Read> for ReadCtx {
            fn to_resolved(self, env: &Env) -> Result<Read, MsgErr> {
                Ok(Read {
                    point: self.point.to_resolved(env)?,
                    payload: self.payload
                })
            }
        }

    }

    pub mod query {
        use std::convert::TryInto;

        use serde::{Deserialize, Serialize};

        use crate::error::MsgErr;
        use crate::version::v0_0_1::command::request::Rc;
        use crate::version::v0_0_1::selector::selector::PointHierarchy;

        #[derive(Debug, Clone, Serialize, Deserialize,Eq,PartialEq)]
        pub enum Query {
            PointKindHierarchy,
        }

        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub enum QueryResult {
            PointKindHierarchy(PointHierarchy),
        }

        impl TryInto<PointHierarchy> for QueryResult {
            type Error = MsgErr;

            fn try_into(self) -> Result<PointHierarchy, MsgErr> {
                match self {
                    QueryResult::PointKindHierarchy(hierarchy) => Ok(hierarchy),
                }
            }
        }

        impl ToString for QueryResult {
            fn to_string(&self) -> String {
                match self {
                    QueryResult::PointKindHierarchy(hierarchy) => hierarchy.to_string(),
                }
            }
        }
    }
}


#[derive(Debug, Clone,Serialize,Deserialize,Eq,PartialEq,Autobox)]
pub enum Command{
    Create(Create),
    Delete(Delete),
    Select(Select),
    Set(Set),
    Get(Get),
    Update(Update),
    Read(Read),
}

impl Command {
    pub fn matches( &self, method: &CmdMethod ) -> Result<(),()>{
        if match self {
            Command::Update(_) => *method == CmdMethod::Update,
            Command::Read(_) => *method == CmdMethod::Read,
            _ => false
        } {
          Ok(())
        } else {
          Err(())
        }
    }
}

pub enum CommandCtx{
    Create(CreateCtx),
    Delete(DeleteCtx),
    Select(SelectCtx),
    Set(SetCtx),
    Get(GetCtx),
    Update(UpdateCtx),
    Read(ReadCtx),
}

pub enum CommandVar {
    Create(CreateVar),
    Delete(DeleteVar),
    Select(SelectVar),
    Set(SetVar),
    Get(GetVar),
    Update(UpdateVar),
    Read(ReadVar)
}


impl FromStr for CommandVar {
    type Err = MsgErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = new_span(s);
        result(all_consuming(command_line)(s))
    }
}

impl ToResolved<Command> for CommandVar {
    fn to_resolved(self, env: &Env) -> Result<Command, MsgErr> {
        let command: CommandCtx = self.to_resolved(env)?;
        command.to_resolved(env)
    }
}

impl ToResolved<CommandCtx> for CommandVar {
    fn to_resolved(self, env: &Env) -> Result<CommandCtx, MsgErr> {
        Ok(match self {
            CommandVar::Create(i) => CommandCtx::Create(i.to_resolved(env)?),
            CommandVar::Select(i) => CommandCtx::Select(i.to_resolved(env)?),
            CommandVar::Set(i) => CommandCtx::Set(i.to_resolved(env)?),
            CommandVar::Get(i) => CommandCtx::Get(i.to_resolved(env)?),
            CommandVar::Delete(i) => CommandCtx::Delete(i.to_resolved(env)?),
            CommandVar::Update(update) => CommandCtx::Update(update.to_resolved(env)?),
            CommandVar::Read(read) => CommandCtx::Read(read.to_resolved(env)?)
        })
    }
}

impl ToResolved<Command> for CommandCtx {
    fn to_resolved(self, env: &Env) -> Result<Command, MsgErr> {
        Ok(match self {
            CommandCtx::Create(i) => Command::Create(i.to_resolved(env)?),
            CommandCtx::Select(i) => Command::Select(i.to_resolved(env)?),
            CommandCtx::Set(i) => Command::Set(i.to_resolved(env)?),
            CommandCtx::Get(i) => Command::Get(i.to_resolved(env)?),
            CommandCtx::Delete(i) => Command::Delete(i.to_resolved(env)?),
            CommandCtx::Update(update) => Command::Update(update.to_resolved(env)?),
            CommandCtx::Read(read) => Command::Read(read.to_resolved(env)?)
        })
    }
}
