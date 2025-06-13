use crate::command::direct::create::KindTemplate;
use crate::err::{ParseErrs, PrintErr};
use crate::loc::{
    ProvisionAffinity, StarKey, ToBaseKind, Version, CONTROL_WAVE_TRAVERSAL_PLAN,
    MECHTRON_WAVE_TRAVERSAL_PLAN, PORTAL_WAVE_TRAVERSAL_PLAN, STAR_WAVE_TRAVERSAL_PLAN,
    STD_WAVE_TRAVERSAL_PLAN,
};
use crate::parse::util::new_span;
use crate::parse::util::result;
use crate::parse::{kind_parts, specific, CamelCase, Domain, SkewerCase};
use crate::particle::traversal::TraversalPlan;
use crate::point::Point;
use crate::selector::{
    KindBaseSelector, KindSelector, Pattern, PointHierarchy, SpecificSelector, SubKindSelector,
    VersionReq,
};
use crate::util::ValuePattern;
use convert_case::{Case, Casing};
use core::str::FromStr;
use nom::combinator::all_consuming;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use indexmap::Equivalent;

impl ToBaseKind for KindParts {
    fn to_base(&self) -> BaseKind {
        self.base.clone()
    }
}

impl Tks for KindParts {
    fn base(&self) -> BaseKind {
        self.base.clone()
    }

    fn sub(&self) -> Option<CamelCase> {
        self.sub.clone()
    }

    fn specific(&self) -> Option<Specific> {
        self.specific.clone()
    }

    fn matches(&self, tks: &dyn Tks) -> bool {
        self.base == tks.base() && self.sub == tks.sub() && self.specific == tks.specific()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct KindParts {
    pub base: BaseKind,
    pub sub: Option<CamelCase>,
    pub specific: Option<Specific>,
}

impl KindParts {
    pub fn root() -> Self {
        Self {
            base: BaseKind::Root,
            sub: None,
            specific: None,
        }
    }
}

impl ToString for KindParts {
    fn to_string(&self) -> String {
        if self.sub.is_some() && self.specific.is_some() {
            format!(
                "{}<{}<{}>>",
                self.base.to_string(),
                self.sub.as_ref().expect("sub").to_string(),
                self.specific.as_ref().expect("specific").to_string()
            )
        } else if self.sub.is_some() {
            format!(
                "{}<{}>",
                self.base.to_string(),
                self.sub.as_ref().expect("sub").to_string()
            )
        } else {
            self.base.to_string()
        }
    }
}

impl FromStr for KindParts {
    type Err = ParseErrs;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (_, kind) = all_consuming(kind_parts)(new_span(s))?;

        Ok(kind)
    }
}

impl KindParts {
    pub fn new(kind: BaseKind, sub: Option<CamelCase>, specific: Option<Specific>) -> Self {
        Self {
            base: kind,
            sub,
            specific,
        }
    }
}

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    Hash,
    strum_macros::Display,
    strum_macros::EnumString,
)]
pub enum BaseKind {
    Root,
    Space,
    UserBase,
    Base,
    User,
    App,
    Mechtron,
    FileStore,
    File,
    Database,
    Repo,
    BundleSeries,
    Bundle,
    Artifact,
    Control,
    Portal,
    Star,
    Driver,
    Global,
    Host,
    Guest,
    Registry,
    WebServer,
    Foundation,
    Dependency,
    Provider,
}

impl BaseKind {
    pub fn bind_point_hierarchy(&self) -> PointHierarchy {
        (match self {
            BaseKind::Star => PointHierarchy::from_str("GLOBAL::repo<Repo>:builtin<BundleSeries>:1.0.0<Bundle>:/<FileStore>star.bind<File<File>>"),
            BaseKind::Driver => PointHierarchy::from_str("GLOBAL::repo<Repo>:builtin<BundleSeries>:1.0.0<Bundle>:/<FileStore>driver.bind<File<File>>"),
            BaseKind::Global => PointHierarchy::from_str("GLOBAL::repo<Repo>:builtin<BundleSeries>:1.0.0<Bundle>:/<FileStore>global.bind<File<File>>"),
            _ => Ok(Self::nothing_bind_point_hierarchy())
        }).map_err(|errs| {
            errs.print();
            errs
        }).unwrap()
    }

    pub fn bind(&self) -> Point {
        self.bind_point_hierarchy().into()
    }

    pub fn nothing_bind() -> Point {
        Self::nothing_bind_point_hierarchy().into()
    }

    pub fn nothing_bind_point_hierarchy() -> PointHierarchy {
        PointHierarchy::from_str("GLOBAL::repo<Repo>:builtin<BundleSeries>:1.0.0<Bundle>:/<FileStore>nothing.bind<File<File>>").unwrap()
    }
}

impl BaseKind {
    pub fn to_skewer(&self) -> SkewerCase {
        SkewerCase::from_str(self.to_string().to_case(Case::Kebab).as_str()).unwrap()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash, strum_macros::Display)]
pub enum Sub {
    None,
    Database(DatabaseSubKind),
    File(FileSubKind),
    Artifact(ArtifactSubKind),
    UserBase(UserBaseSubKind),
    Star(StarSub),
}

impl Sub {
    pub fn sub_kind(self) -> SubKind {
        match self {
            Sub::None => SubKind::None,
            Sub::Database(_) => SubKind::Database,
            Sub::File(_) => SubKind::File,
            Sub::Artifact(_) => SubKind::Artifact,
            Sub::UserBase(_) => SubKind::UserBase,
            Sub::Star(_) => SubKind::Star,
        }
    }
}

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    Hash,
    strum_macros::Display,
    strum_macros::EnumString,
)]
pub enum SubKind {
    None,
    Database,
    File,
    Artifact,
    UserBase,
    Star,
}

impl Sub {
    pub fn to_camel_case(&self) -> Option<CamelCase> {
        match self {
            Sub::None => None,
            Sub::Database(d) => Some(CamelCase::from_str(d.to_string().as_str()).unwrap()),
            Sub::File(x) => Some(CamelCase::from_str(x.to_string().as_str()).unwrap()),
            Sub::Artifact(x) => Some(CamelCase::from_str(x.to_string().as_str()).unwrap()),
            Sub::UserBase(x) => Some(CamelCase::from_str(x.to_string().as_str()).unwrap()),
            Sub::Star(x) => Some(CamelCase::from_str(x.to_string().as_str()).unwrap()),
        }
    }

    pub fn specific(&self) -> Option<&Specific> {
        match self {
            Sub::Database(sub) => sub.specific(),
            Sub::UserBase(sub) => sub.specific(),
            _ => None,
        }
    }
}

impl Sub {
    pub fn to_skewer(&self) -> SkewerCase {
        SkewerCase::from_str(self.to_string().to_case(Case::Kebab).as_str()).unwrap()
    }
}

impl Into<Option<CamelCase>> for Sub {
    fn into(self) -> Option<CamelCase> {
        match self {
            Sub::None => None,
            Sub::Database(d) => d.into(),
            Sub::File(f) => f.into(),
            Sub::Artifact(a) => a.into(),
            Sub::UserBase(u) => u.into(),
            Sub::Star(s) => s.into(),
        }
    }
}

impl Into<Option<String>> for Sub {
    fn into(self) -> Option<String> {
        match self {
            Sub::None => None,
            Sub::Database(d) => d.into(),
            Sub::File(f) => f.into(),
            Sub::Artifact(a) => a.into(),
            Sub::UserBase(u) => u.into(),
            Sub::Star(s) => s.into(),
        }
    }
}

impl ToBaseKind for BaseKind {
    fn to_base(&self) -> BaseKind {
        self.clone()
    }
}

impl TryFrom<CamelCase> for BaseKind {
    type Error = ParseErrs;

    fn try_from(base: CamelCase) -> Result<Self, Self::Error> {
        Ok(BaseKind::from_str(base.as_str())?)
    }
}

/// Kind defines the behavior and properties of a Particle.  Each particle has a Kind.
/// At minimum a Kind must have a BaseKind, it can also have a SubKind and a Specific.
/// A Particle's complete Kind definition is used to match it with a Driver in the HyperVerse
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash, strum_macros::Display)]
pub enum Kind {
    Root,
    Space,
    User,
    App,
    Mechtron,
    FileStore,
    Repo,
    BundleSeries,
    Bundle,
    Control,
    Portal,
    Driver,
    #[strum(to_string = "File<{0}>")]
    File(FileSubKind),
    #[strum(to_string = "Artifact<{0}>")]
    Artifact(ArtifactSubKind),
    #[strum(to_string = "Database<{0}>")]
    Database(DatabaseSubKind),
    Base,
    #[strum(to_string = "UserBase<{0}>")]
    UserBase(UserBaseSubKind),
    Star(StarSub),
    Global,
    Host,
    Guest,
    Registry,
    WebServer,
    Foundation,
    Dependency,
    Provider,
}

impl ToBaseKind for Kind {
    fn to_base(&self) -> BaseKind {
        match self {
            Kind::Root => BaseKind::Root,
            Kind::Space => BaseKind::Space,
            Kind::User => BaseKind::User,
            Kind::App => BaseKind::App,
            Kind::Mechtron => BaseKind::Mechtron,
            Kind::FileStore => BaseKind::FileStore,
            Kind::BundleSeries => BaseKind::BundleSeries,
            Kind::Bundle => BaseKind::Bundle,
            Kind::Control => BaseKind::Control,
            Kind::Portal => BaseKind::Portal,
            Kind::UserBase(_) => BaseKind::UserBase,
            Kind::File(_) => BaseKind::File,
            Kind::Artifact(_) => BaseKind::Artifact,
            Kind::Database(_) => BaseKind::Database,
            Kind::Base => BaseKind::Base,
            Kind::Repo => BaseKind::Repo,
            Kind::Star(_) => BaseKind::Star,
            Kind::Driver => BaseKind::Driver,
            Kind::Global => BaseKind::Global,
            Kind::Host => BaseKind::Host,
            Kind::Guest => BaseKind::Guest,
            Kind::Registry => BaseKind::Registry,
            Kind::WebServer => BaseKind::WebServer,
            Kind::Foundation => BaseKind::Foundation,
            Kind::Dependency => BaseKind::Dependency,
            Kind::Provider => BaseKind::Provider,
        }
    }
}

impl Kind {
    pub fn opt_sub(&self) -> Option<Sub> {
        match &self.sub() {
            Sub::None => None,
            s => Some(s.clone()),
        }
    }

    pub fn to_template(&self) -> KindTemplate {
        KindTemplate {
            base: self.to_base(),
            sub: self.sub().to_camel_case(),
            specific: self.specific_selector(),
        }
    }

    pub fn provision_affinity(&self) -> ProvisionAffinity {
        match self.to_base() {
            BaseKind::Base => ProvisionAffinity::Local,
            _ => ProvisionAffinity::Wrangle,
        }
    }

    pub fn is_auto_provision(&self) -> bool {
        match self {
            Kind::Bundle => true,
            Kind::Artifact(_) => true,
            Kind::Mechtron => true,
            Kind::Host => true,
            _ => false,
        }
    }

    pub fn as_point_segments(&self) -> String {
        if Sub::None != self.sub() {
            if let Some(specific) = self.specific() {
                format!(
                    "{}:{}:{}",
                    self.to_base().to_skewer().to_string(),
                    self.sub().to_skewer().to_string(),
                    specific.to_string()
                )
            } else {
                format!(
                    "{}:{}",
                    self.to_base().to_skewer().to_string(),
                    self.sub().to_skewer().to_string()
                )
            }
        } else {
            format!("{}", self.to_base().to_skewer().to_string())
        }
    }

    pub fn sub(&self) -> Sub {
        match self {
            Kind::File(s) => s.clone().into(),
            Kind::Artifact(s) => s.clone().into(),
            Kind::Database(s) => s.clone().into(),
            Kind::UserBase(s) => s.clone().into(),
            Kind::Star(s) => s.clone().into(),
            _ => Sub::None,
        }
    }

    pub fn specific(&self) -> Option<Specific> {
        let sub = self.sub();
        sub.specific().cloned()
    }

    pub fn specific_selector(&self) -> Option<SpecificSelector> {
        match self.specific() {
            None => None,
            Some(specific) => Some(specific.to_selector()),
        }
    }

    pub fn wave_traversal_plan(&self) -> &TraversalPlan {
        match self {
            Kind::Mechtron => &MECHTRON_WAVE_TRAVERSAL_PLAN,
            Kind::Portal => &PORTAL_WAVE_TRAVERSAL_PLAN,
            Kind::Control => &CONTROL_WAVE_TRAVERSAL_PLAN,
            Kind::Star(_) => &STAR_WAVE_TRAVERSAL_PLAN,
            _ => &STD_WAVE_TRAVERSAL_PLAN,
        }
    }
}

impl TryFrom<KindParts> for Kind {
    type Error = ParseErrs;

    fn try_from(value: KindParts) -> Result<Self, Self::Error> {
        Ok(match value.base {
            BaseKind::Database => {
                match value.sub.ok_or("Database<?> requires a Sub Kind")?.as_str() {
                    "Relational" => Kind::Database(DatabaseSubKind::Relational(
                        value
                            .specific
                            .ok_or("Database<Relational<?>> requires a Specific")?,
                    )),
                    what => {
                        return Err(ParseErrs::from(format!(
                            "unexpected Database SubKind '{}'",
                            what
                        )));
                    }
                }
            }
            BaseKind::UserBase => {
                match value.sub.ok_or("UserBase<?> requires a Sub Kind")?.as_str() {
                    "OAuth" => Kind::UserBase(UserBaseSubKind::OAuth(
                        value
                            .specific
                            .ok_or("UserBase<OAuth<?>> requires a Specific")?,
                    )),
                    what => {
                        return Err(ParseErrs::from(format!(
                            "unexpected Database SubKind '{}'",
                            what
                        )));
                    }
                }
            }
            BaseKind::Base => Kind::Base,
            BaseKind::File => Kind::File(FileSubKind::from_str(
                value.sub.ok_or("File<?> requires a Sub Kind")?.as_str(),
            )?),
            BaseKind::Artifact => Kind::Artifact(ArtifactSubKind::from_str(
                value.sub.ok_or("Artifact<?> requires a sub kind")?.as_str(),
            )?),

            BaseKind::Star => Kind::Star(StarSub::from_str(
                value.sub.ok_or("Star<?> requires a sub kind")?.as_str(),
            )?),

            BaseKind::Root => Kind::Root,
            BaseKind::Space => Kind::Space,
            BaseKind::User => Kind::User,
            BaseKind::App => Kind::App,
            BaseKind::Mechtron => Kind::Mechtron,
            BaseKind::FileStore => Kind::FileStore,

            BaseKind::BundleSeries => Kind::BundleSeries,
            BaseKind::Bundle => Kind::Bundle,
            BaseKind::Control => Kind::Control,
            BaseKind::Portal => Kind::Portal,
            BaseKind::Repo => Kind::Repo,
            BaseKind::Driver => Kind::Driver,
            BaseKind::Global => Kind::Global,
            BaseKind::Host => Kind::Host,
            BaseKind::Guest => Kind::Guest,
            BaseKind::Registry => Kind::Registry,
            BaseKind::WebServer => Kind::WebServer,
            BaseKind::Foundation => Kind::Foundation,
            BaseKind::Dependency => Kind::Dependency,
            BaseKind::Provider => Kind::Provider,
        })
    }
}

/// Stands for "Type, Kind, Specific"
pub trait Tks {
    fn base(&self) -> BaseKind;
    fn sub(&self) -> Option<CamelCase>;
    fn specific(&self) -> Option<Specific>;
    fn matches(&self, tks: &dyn Tks) -> bool;
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Hash,
    Serialize,
    Deserialize,
    strum_macros::Display,
    strum_macros::EnumString,
    strum_macros::EnumIter,
)]
pub enum StarSub {
    Central,
    Super, // Wrangles nearby Stars... manages Assigning Particles to Stars, Moving, Icing, etc.
    Nexus, // Relays Waves from Star to Star
    Maelstrom, // Where executables are run
    Scribe, // requires durable filesystem (Artifact Bundles, Files...)
    Jump, // for entry into the Mesh/Fabric for an external connection (client ingress... http for example)
    Fold, // exit from the Mesh.. maintains connections etc to Databases, Keycloak, etc.... Like A Space Fold out of the Fabric..
    Machine, // every Machine has one and only one Machine star... it handles messaging for the Machine
}

impl StarSub {
    pub fn to_selector(self) -> KindSelector {
        KindSelector {
            base: KindBaseSelector::Exact(BaseKind::Star),
            sub: SubKindSelector::Exact(self.to_camel_case()),
            specific: ValuePattern::Always,
        }
    }

    pub fn to_camel_case(&self) -> CamelCase {
        CamelCase::from_str(self.to_string().as_str()).unwrap()
    }

    pub fn is_forwarder(&self) -> bool {
        match self {
            StarSub::Nexus => true,
            StarSub::Central => false,
            StarSub::Super => true,
            StarSub::Maelstrom => true,
            StarSub::Scribe => true,
            StarSub::Jump => true,
            StarSub::Fold => true,
            StarSub::Machine => false,
        }
    }

    pub fn can_be_wrangled(&self) -> bool {
        match self {
            StarSub::Nexus => false,
            StarSub::Machine => false,
            _ => true,
        }
    }
}

impl Into<Sub> for StarSub {
    fn into(self) -> Sub {
        Sub::Star(self)
    }
}

impl Into<Option<CamelCase>> for StarSub {
    fn into(self) -> Option<CamelCase> {
        Some(CamelCase::from_str(self.to_string().as_str()).unwrap())
    }
}

impl Into<Option<String>> for StarSub {
    fn into(self) -> Option<String> {
        Some(self.to_string())
    }
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Hash,
    Serialize,
    Deserialize,
    strum_macros::Display,
    strum_macros::EnumIter,
)]
pub enum UserBaseSubKindBase {
    OAuth,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, strum_macros::Display)]
pub enum UserBaseSubKind {
    #[strum(to_string = "OAuth<{0}>")]
    OAuth(Specific),
}

impl UserBaseSubKind {
    pub fn specific(&self) -> Option<&Specific> {
        match self {
            UserBaseSubKind::OAuth(specific) => Option::Some(specific),
        }
    }
}

impl Into<Sub> for UserBaseSubKind {
    fn into(self) -> Sub {
        Sub::UserBase(self)
    }
}

impl Into<Option<CamelCase>> for UserBaseSubKind {
    fn into(self) -> Option<CamelCase> {
        Some(CamelCase::from_str(self.to_string().as_str()).unwrap())
    }
}

impl Into<Option<String>> for UserBaseSubKind {
    fn into(self) -> Option<String> {
        Some(self.to_string())
    }
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Hash,
    Serialize,
    Deserialize,
    strum_macros::Display,
    strum_macros::EnumString,
    strum_macros::EnumIter,
)]
pub enum FileSubKind {
    File,
    Dir,
}

impl Into<Sub> for FileSubKind {
    fn into(self) -> Sub {
        Sub::File(self)
    }
}

impl Into<Option<CamelCase>> for FileSubKind {
    fn into(self) -> Option<CamelCase> {
        Some(CamelCase::from_str(self.to_string().as_str()).unwrap())
    }
}

impl Into<Option<String>> for FileSubKind {
    fn into(self) -> Option<String> {
        Some(self.to_string())
    }
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Hash,
    Serialize,
    Deserialize,
    strum_macros::Display,
    strum_macros::EnumString,
    strum_macros::EnumIter,
)]
pub enum ArtifactSubKind {
    Raw,
    ParticleConfig,
    Bind,
    Wasm,
    Dir,
}

impl Into<Sub> for ArtifactSubKind {
    fn into(self) -> Sub {
        Sub::Artifact(self)
    }
}

impl Into<Option<CamelCase>> for ArtifactSubKind {
    fn into(self) -> Option<CamelCase> {
        Some(CamelCase::from_str(self.to_string().as_str()).unwrap())
    }
}

impl Into<Option<String>> for ArtifactSubKind {
    fn into(self) -> Option<String> {
        Some(self.to_string())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, strum_macros::Display)]
pub enum DatabaseSubKind {
    #[strum(to_string = "Relational<{0}>")]
    Relational(Specific),
}

impl DatabaseSubKind {
    pub fn specific(&self) -> Option<&Specific> {
        match self {
            DatabaseSubKind::Relational(specific) => Some(specific),
        }
    }
}

impl Into<Sub> for DatabaseSubKind {
    fn into(self) -> Sub {
        Sub::Database(self)
    }
}

impl Into<Option<CamelCase>> for DatabaseSubKind {
    fn into(self) -> Option<CamelCase> {
        Some(CamelCase::from_str(self.to_string().as_str()).unwrap())
    }
}

impl Into<Option<String>> for DatabaseSubKind {
    fn into(self) -> Option<String> {
        Some(self.to_string())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct StarStub {
    pub key: StarKey,
    pub kind: StarSub,
}

impl StarStub {
    pub fn new(key: StarKey, kind: StarSub) -> Self {
        Self { key, kind }
    }
}


/// A Specific is used to associate [Type](s) with an`Exact` definition(s)
///
/// The Specific segment schema is designed to avoid naming collisions and follows
/// this pattern:
/// `provider.url:vendor.url:product:variant:product-semver:specific-semver`
///
/// `Segments`:
/// * `provider`  The domain name of the person or entity that created and published the definitions
///                and configurations that enabled `Starlane` to bind with the `product`.
///                Remember the `provider` provides the definition (particularly the `BindConfig`
///                required for any `Type` to be used by `Starlane`) The `provider` may or may
///                not have anything to do with the 3rd party software itself.
///
/// * `vendor`     The domain name of the person or entity that produces the software regardless
///                if he is even aware of its use to extend.
///
///                To illustrate the need for the two domain scopes for `provider` and `vendor`
///                consider two cases:
///                1) A software development company with the web domain `my-software-company.com`
///                   publishes its in house products making him a `vendor` and he also publishes
///                   a Starlane Specific definition for his product making him also a `provider`
///                   In this first case the provider::vendor segments would be
///                   `my-software-company.com:my-software-company.com`
///
///                2) In the second instance Red Had publishes its identity and authorization product:
///                   `KeyCloak` making the `vendor` `redhat.com`.    Another person entirely likes
///                   `KeyCloak` and wants to use it with `Starlane`... his domain name is `personal-site.com`
///                   he publishes the Specific definition so `Starlane` can use `KeyCloak` to back
///                   the `UserBase` `Class` he is the `provider` of the `Specific` definition and
///                   decided that the best fit for identifying `KeyCloak's` vendor domain was
///                  `redhat.com`.   In the second scenario the provider::vendor segments would look
///                   like:
///                   `personal-site.com:redhat.com`
///
/// * `product`   - ...Should be rather straightforward.  Companies may create multiple products
///                 and therefor the product is employed to further  distinguish the `Specific`
///
/// * `variant`   - May not apply in every case but sometimes companies provide different variants
///                 of a product.  For example the variants might be `Community` and `Enterprise`
///
/// * `product-semver`  - A semver of the product version.
///
/// * `specific-semver` - A semver of this `Specific` definition
///
/// ## Example:
/// `hub.starlane.io:postgres.org:postgres:gis:8.0.0:1.0.1`
///
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub struct Specific {
    pub provider: Domain,
    pub vendor: Domain,
    pub product: SkewerCase,
    pub variant: SkewerCase,
    pub version: Version,
}

impl Equivalent<Specific> for &Specific {
    fn equivalent(&self, other: &Specific) -> bool {
        *self == other
    }
}

impl Into<Specific> for &Specific {
    fn into(self) -> Specific {
       self.clone()
    }
}

impl Specific {
    pub fn to_selector(&self) -> SpecificSelector {
        SpecificSelector::from_str(self.to_string().as_str()).unwrap()
    }
}

impl Display for Specific {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let string = format!(
            "{}:{}:{}:{}:{}",
            self.provider,
            self.vendor,
            self.product,
            self.variant,
            self.version.to_string()
        );
        f.write_str(string.as_str())
    }
}

impl FromStr for Specific {
    type Err = ParseErrs;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        result(specific(new_span(s)))
    }
}

impl TryInto<SpecificSelector> for Specific {
    type Error = ParseErrs;

    fn try_into(self) -> Result<SpecificSelector, Self::Error> {
        Ok(SpecificSelector {
            provider: Pattern::Exact(self.provider),
            vendor: Pattern::Exact(self.vendor),
            product: Pattern::Exact(self.product),
            variant: Pattern::Exact(self.variant),
            version: VersionReq::from_str(self.version.to_string().as_str())?,
        })
    }
}

#[cfg(test)]
pub mod test {
    use crate::err::{ParseErrs, PrintErr};
    use crate::kind::{FileSubKind, Kind, StarSub};
    use crate::parse::util::{new_span, result};
    use crate::parse::{file_point_kind_segment, point_kind_hierarchy};
    use crate::selector::{KindSelector, PointHierarchy};
    use crate::util::ValueMatcher;
    use nom::combinator::all_consuming;
    use std::str::FromStr;

    #[test]
    pub fn selector() -> Result<(), ParseErrs> {
        let kind = Kind::Star(StarSub::Fold);
        let selector = KindSelector::from_str("<Star<Fold>>")?;
        assert!(selector.is_match(&kind).is_ok());

        Ok(())
    }

    #[test]
    pub fn star_bind() {
        let s = "GLOBAL::repo<Repo>:builtin<BundleSeries>:1.0.0<Bundle>:/<FileStore>star.bind<File<File>>";
        let string = s.to_string();
        let s = new_span(s);

        let (_, hierarchy) = point_kind_hierarchy(s).unwrap();
        let v = hierarchy.to_string();

        assert_eq!(v, string);
    }

    #[test]
    pub fn file_bind() {
        println!(
            "File<File> == {}",
            Kind::File(FileSubKind::File).to_string()
        );
        let s = "star.bind<File<File>>";
        match result(all_consuming(file_point_kind_segment)(new_span(s))) {
            Ok(ok) => {
                println!("filePoint seg: '{}'", ok.to_string());
            }
            Err(err) => {
                err.print();
                assert!(false)
            }
        }
    }
    #[test]
    pub fn star_bind_from_str() {
        let s = "GLOBAL::repo<Repo>:builtin<BundleSeries>:1.0.0<Bundle>:/<FileStore>star.bind<File<File>>";
        match PointHierarchy::from_str(s) {
            Ok(hierarchy) => {
                println!("HIERARCHY from_str {}", hierarchy.to_string());
                let v = s.to_string();
                assert_eq!(v, hierarchy.to_string());
            }
            Err(err) => {
                err.print();
                panic!("from_str does not work!")
            }
        }
    }
}
