use core::fmt::{Debug, Formatter};
use core::str::FromStr;
use std::ops::Deref;

use nom::combinator::all_consuming;
use serde::de::{DeserializeOwned, Error, Visitor};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use crate::command::direct::select::SubSelect;
use crate::err::ParseErrs;
use crate::kind::{BaseKind, Kind, KindParts, Specific, Sub, SubKind};
use crate::loc::{Layer, ToBaseKind, Topic, VarVal, Variable, Version};
use crate::parse::util::result;
use crate::parse::util::{new_span, Trace};
use crate::parse::{
    consume_hierarchy, kind_selector, point_segment_selector, point_selector, specific_selector,
    CamelCase, Env,
};
use crate::point::{Point, PointCtx, PointDef, PointSeg, PointVar, RouteSeg};
use crate::substance::{
    CallWithConfigDef, Substance, SubstanceFormat, SubstanceKind, SubstancePattern,
    SubstancePatternCtx, SubstancePatternDef,
};
use crate::util::{ToResolved, ValueMatcher, ValuePattern};
use crate::SpaceErr;
use specific::{ProductSelector, ProviderSelector, VariantSelector, VendorSelector};

pub type PointSegKindHop = HopDef<PointSegSelector, KindSelector>;
pub type PointSegKindHopCtx = HopDef<PointSegSelectorCtx, KindSelector>;
pub type PointSegKindHopVar = HopDef<PointSegSelectorVar, KindSelector>;

pub type Selector = SelectorDef<PointSegKindHop>;
pub type SelectorCtx = SelectorDef<PointSegKindHopCtx>;
pub type SelectorVar = SelectorDef<PointSegKindHopVar>;

pub type PointSelector = Selector;

pub type KindSelector = KindSelectorDef<KindBaseSelector, SubKindSelector, SpecificSelector>;
pub type KindSelectorVar =
    KindSelectorDef<VarVal<KindBaseSelector>, VarVal<SubKindSelector>, VarVal<SpecificSelector>>;

impl PartialEq<Kind> for KindSelector {
    fn eq(&self, kind: &Kind) -> bool {
        self.is_match(kind).is_ok()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct KindSelectorDef<GenericKindSelector, GenericSubKindSelector, SpecificSelector> {
    pub base: GenericKindSelector,
    pub sub: GenericSubKindSelector,
    pub specific: ValuePattern<SpecificSelector>,
}

impl ValueMatcher<Kind> for KindSelector {
    fn is_match(&self, kind: &Kind) -> Result<(), ()> {
        let base = kind.to_base();
        let sub = kind.sub().to_camel_case();
        let specific = &kind.specific();
        match self.base.is_match(&base).is_ok()
            && self.sub.is_match(&sub).is_ok()
            && self.specific.is_match_opt(specific).is_ok()
        {
            true => Ok(()),
            false => Err(()),
        }
    }
}

impl KindSelector {
    pub fn new(
        kind: KindBaseSelector,
        sub_kind: SubKindSelector,
        specific: ValuePattern<SpecificSelector>,
    ) -> Self {
        Self {
            base: kind,
            sub: sub_kind,
            specific,
        }
    }

    pub fn from_base(base: BaseKind) -> Self {
        Self {
            base: KindBaseSelector::Exact(base),
            sub: SubKindSelector::Always,
            specific: ValuePattern::Always,
        }
    }

    pub fn as_point_segments(&self) -> Result<String, SpaceErr> {
        match &self.base {
            KindBaseSelector::Always => Err(SpaceErr::server_error(
                "cannot turn a base wildcard kind into point segments",
            )),
            KindBaseSelector::Exact(e) => Ok(e.to_skewer().to_string()),
            KindBaseSelector::Never => Err(SpaceErr::server_error(
                "cannot turn a base never kind into point segments",
            )),
        }
    }
}

impl FromStr for KindSelector {
    type Err = ParseErrs;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(result(kind_selector(new_span(s)))?)
    }
}

impl ToString for KindSelector {
    fn to_string(&self) -> String {
        format!(
            "{}<{}<{}>>",
            self.base.to_string(),
            match &self.sub {
                SubKindSelector::Always => "*".to_string(),
                SubKindSelector::Exact(sub) => sub.to_string(),
                SubKindSelector::None => "".to_string(),
                SubKindSelector::Never => "!".to_string(),
            },
            self.specific.to_string()
        )
    }
}

impl KindSelector {
    pub fn any() -> Self {
        Self {
            base: KindBaseSelector::Always,
            sub: SubKindSelector::Always,
            specific: ValuePattern::Always,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash, strum_macros::Display)]
pub enum SubKindSelector {
    #[strum(to_string = "")]
    None,
    #[strum(to_string = "*")]
    Always,
    #[strum(to_string = "!")]
    Never,
    #[strum(to_string = "{0}")]
    Exact(CamelCase),
}

impl ValueMatcher<CamelCase> for SubKindSelector {
    fn is_match(&self, other: &CamelCase) -> Result<(), ()> {
        match self {
            SubKindSelector::None => Ok(()),
            SubKindSelector::Always => Ok(()),
            SubKindSelector::Never => Err(()),
            SubKindSelector::Exact(sub) if sub == other => Ok(()),
            _ => Err(()),
        }
    }
}

impl ValueMatcher<SubKind> for SubKindSelector {
    fn is_match(&self, other: &SubKind) -> Result<(), ()> {
        match self {
            SubKindSelector::None => Ok(()),
            SubKindSelector::Always => Ok(()),
            SubKindSelector::Never => Err(()),
            SubKindSelector::Exact(sub) if sub.to_string() == other.to_string() => Ok(()),
            _ => Err(()),
        }
    }
}

impl ValueMatcher<Option<CamelCase>> for SubKindSelector {
    fn is_match(&self, other: &Option<CamelCase>) -> Result<(), ()> {
        match self {
            SubKindSelector::None => Ok(()),
            SubKindSelector::Always => Ok(()),
            SubKindSelector::Never => Err(()),
            SubKindSelector::Exact(sub) => match other.as_ref() {
                None => Err(()),
                Some(other) if sub == other => Ok(()),
                _ => Err(()),
            },
        }
    }
}
impl ValueMatcher<Option<SubKind>> for SubKindSelector {
    fn is_match(&self, other: &Option<SubKind>) -> Result<(), ()> {
        match self {
            SubKindSelector::None => Ok(()),
            SubKindSelector::Always => Ok(()),
            SubKindSelector::Never => Err(()),
            SubKindSelector::Exact(sub) => match other.as_ref() {
                None => Err(()),
                Some(other) if sub.to_string() == other.to_string() => Ok(()),
                _ => Err(()),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct SelectorDef<Hop> {
    /// this is a temporary hack which will hopefully not be necessary when types 2.0 become available
    pub(crate) always: bool,
    pub hops: Vec<Hop>,
}

impl PointSelector {
    pub fn always() -> Self {
        Self {
            always: true,
            hops: vec![],
        }
    }
}

/*
impl PartialEq<Point> for SelectorDef<PointSegKindHop> {
    fn eq(&self, other: &Point) -> bool {
        let hierarchy = other.clone().to_opt_hierarchy();
        self.matches_found(&hierarchy)
    }
}

 */

/*
pub type Selector = SelectorDef<PointSegKindHop>;
pub type SelectorCtx = SelectorDef<SelHopCtx>;
pub type SelectorVar = SelectorDef<SelHopVar>;

 */

impl ToResolved<Selector> for Selector {
    fn to_resolved(self, env: &Env) -> Result<Selector, ParseErrs> {
        Ok(self)
    }
}

/*
impl ValueMatcher<PointKindSeg> for PointSegHop {
    fn is_match(&self, x: &PointKindSeg) -> Result<(), ()> {
        if self.segment_selector.is_match(&x.segment) && self.kind_selector.is_match(&x.kind).is_ok() {
            Ok(())
        } else {
            Err(())
        }
    }
}

 */

impl ValueMatcher<PointSeg> for PointSegKindHop {
    fn is_match(&self, x: &PointSeg) -> Result<(), ()> {
        if self.segment_selector.is_match(x) {
            Ok(())
        } else {
            Err(())
        }
    }
}

impl ValueMatcher<PointKindSeg> for PointSegKindHop {
    fn is_match(&self, x: &PointKindSeg) -> Result<(), ()> {
        if self.segment_selector.is_match(&x.segment)
            && self.kind_selector.is_match(&x.kind).is_ok()
        {
            Ok(())
        } else {
            Err(())
        }
    }
}

impl ValueMatcher<PointKindSegOpt> for PointSegKindHop {
    fn is_match(&self, x: &PointKindSegOpt) -> Result<(), ()> {
        if self.segment_selector.is_match(&x.segment) {
            if let Some(ref k) = x.kind {
                self.kind_selector.is_match(k)
            } else {
                Ok(())
            }
        } else {
            Err(())
        }
    }
}

impl FromStr for SelectorDef<PointSegKindHop> {
    type Err = ParseErrs;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (_, rtn) = all_consuming(point_selector)(new_span(s))?;
        Ok(rtn)
    }
}

impl ValueMatcher<Point> for Selector {
    fn is_match(&self, point: &Point) -> Result<(), ()> {
        if self.always {
            return Ok(());
        }

        match self.matches_found(point) {
            true => Ok(()),
            false => Err(()),
        }
    }
}

impl Selector {
    fn consume(&self) -> Option<Selector> {
        if self.hops.is_empty() {
            Option::None
        } else {
            let mut hops = self.hops.clone();
            hops.remove(0);
            Option::Some(Selector {
                hops,
                always: false,
            })
        }
    }

    pub fn matches_root(&self) -> bool {
        if self.hops.is_empty() {
            true
        } else if self.hops.len() == 1 {
            let hop = self.hops.first().unwrap();
            if PointSegSelector::InclusiveAny == hop.segment_selector
                || PointSegSelector::InclusiveRecursive == hop.segment_selector
            {
                hop.kind_selector.is_match(&Kind::Root).is_ok()
            } else {
                false
            }
        } else {
            false
        }
    }

    pub fn is_root(&self) -> bool {
        self.hops.is_empty()
    }

    pub fn is_final(&self) -> bool {
        self.hops.len() == 1
    }

    pub fn query_root(&self) -> Point {
        let mut segments = vec![];
        for hop in &self.hops {
            if let PointSegSelector::Exact(exact) = &hop.segment_selector {
                if hop.inclusive {
                    break;
                }
                match exact {
                    ExactPointSeg::PointSeg(seg) => {
                        segments.push(seg.clone());
                    }
                    ExactPointSeg::Version(version) => {
                        segments.push(PointSeg::Version(version.clone()));
                    }
                }
            } else {
                break;
            }
        }

        Point {
            route: RouteSeg::This,
            segments,
        }
    }

    pub fn sub_select_hops(&self) -> Vec<PointSegKindHop> {
        let mut hops = self.hops.clone();
        let query_root_segments = self.query_root().segments.len();
        for _ in 0..query_root_segments {
            hops.remove(0);
        }
        hops
    }

    pub fn matches_found<H>(&self, hierarchy: &H) -> bool
    where
        PointHierarchyOpt: for<'a> From<&'a H>,
        BaseKind: Clone,
        KindParts: Clone,
    {
        let opt = PointHierarchyOpt::from(hierarchy);
        self.matches_found_opt(&opt)
    }

    pub fn matches_found_opt(&self, hierarchy: &PointHierarchyOpt) -> bool
    where
        BaseKind: Clone,
        KindParts: Clone,
    {
        if hierarchy.is_root() && self.is_root() {
            return true;
        }

        if hierarchy.segments.is_empty() || self.hops.is_empty() {
            return false;
        }

        let hop = self.hops.first().expect("hop");
        let seg = hierarchy.segments.first().expect("segment");

        /*
                    if hierarchy.segments.len() < self.hops.len() {
                        // if a hop is 'inclusive' then this will match to true.  We do this for cases like:
                        // localhost+:**   // Here we want everything under localhost INCLUDING localhost to be matched
        println!("hop: {}", hop.to_string());
        println!("seg: {}", seg.to_string());
                        if hop.inclusive && hop.matches(&seg) {
                            return true;
                        } else {
                            return false;
                        }
                    }

                     */

        if hierarchy.is_final() && self.is_final() {
            // this is the final hop & segment if they match, everything matches!
            hop.is_match(seg).is_ok()
        } else if hierarchy.is_root() {
            false
        } else if self.is_root() {
            false
        } else if hierarchy.is_final() {
            // we still have hops that haven't been matched, and we are all out of path... but we have a weird rule
            // if a hop is 'inclusive' then this will match to true.  We do this for cases like:
            // localhost+:**   // Here we want everything under localhost INCLUDING localhost to be matched
            if hop.inclusive && hop.is_match(seg).is_ok() {
                true
            } else {
                false
            }
        }
        // special logic is applied to recursives **
        else if hop.segment_selector.is_recursive() && self.hops.len() >= 2 {
            // a Recursive is similar to an Any in that it will match anything, however,
            // it is not consumed until the NEXT segment matches...
            let next_hop = self.hops.get(1).expect("next<Hop>");
            if next_hop.is_match(seg).is_ok() {
                // since the next hop after the recursive matches, we consume the recursive and continue hopping
                // this allows us to make matches like:
                // space.org:**:users ~ space.org:many:silly:dirs:users
                self.consume()
                    .expect("PointSelector")
                    .matches_found_opt(&hierarchy.consume().expect("AddressKindPath"))
            } else {
                // the NEXT hop does not match, therefore we do NOT consume() the current hop
                self.matches_found_opt(&hierarchy.consume().expect("AddressKindPath"))
            }
        } else if hop.segment_selector.is_recursive() && hierarchy.is_final() {
            hop.is_match(hierarchy.segments.last().expect("segment"))
                .is_ok()
        } else if hop.segment_selector.is_recursive() {
            hop.is_match(hierarchy.segments.last().expect("segment"))
                .is_ok()
                && self.matches_found_opt(&hierarchy.consume().expect("hierarchy"))
        } else if hop.is_match(seg).is_ok() {
            // in a normal match situation, we consume the hop and move to the next one
            self.consume()
                .expect("AddressTksPattern")
                .matches_found_opt(&hierarchy.consume().expect("AddressKindPath"))
        } else {
            false
        }
    }
}

impl ToString for Selector {
    fn to_string(&self) -> String {
        let mut rtn = String::new();
        for (index, hop) in self.hops.iter().enumerate() {
            rtn.push_str(hop.to_string().as_str());
            if index < self.hops.len() - 1 {
                rtn.push_str(":");
            }
        }
        rtn
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct VersionReq {
    pub version: semver::VersionReq,
}

impl VersionReq {
    pub fn always() -> VersionReq {
        VersionReq::from_str("").expect("expected ANY match for VersionReq")
    }
}

impl Deref for VersionReq {
    type Target = semver::VersionReq;

    fn deref(&self) -> &Self::Target {
        &self.version
    }
}

impl Serialize for VersionReq {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.version.to_string().as_str())
    }
}

impl<'de> Deserialize<'de> for VersionReq {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(VersionReqVisitor)
    }
}

struct VersionReqVisitor;

impl<'de> Visitor<'de> for VersionReqVisitor {
    type Value = VersionReq;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        formatter.write_str("SemVer version requirement")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        match VersionReq::from_str(v) {
            Ok(version) => Ok(version),
            Err(error) => {
                //Err(de::Error::custom(error.to_string() ))
                Err(de::Error::invalid_type(de::Unexpected::Str(v), &self))
            }
        }
    }
}

impl ToString for VersionReq {
    fn to_string(&self) -> String {
        self.version.to_string()
    }
}

impl TryInto<semver::VersionReq> for VersionReq {
    type Error = SpaceErr;

    fn try_into(self) -> Result<semver::VersionReq, Self::Error> {
        Ok(self.version)
    }
}

impl FromStr for VersionReq {
    type Err = ParseErrs;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let version = semver::VersionReq::from_str(s)?;
        Ok(Self { version })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub enum PointSegSelector {
    InclusiveAny,       // +:*  // includes Root if it's the first segment
    InclusiveRecursive, // +:** // includes Root if its the first segment
    Any,                // *
    Recursive,          // **
    Exact(ExactPointSeg),
    Version(VersionReq),
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub enum PointSegSelectorVar {
    InclusiveAny,       // +:*  // includes Root if it's the first segment
    InclusiveRecursive, // +:** // includes Root if it's the first segment
    Any,                // *
    Recursive,          // **
    Exact(ExactPointSeg),
    Version(VersionReq),
    Var(Variable),
    Working(Trace),
    Pop(Trace),
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub enum PointSegSelectorCtx {
    /// +:*  // includes Root if it's the first segment
    InclusiveAny,
    /// +:** // includes Root if it's the first segment
    InclusiveRecursive,
    /// *
    Any,
    /// **
    Recursive,
    Exact(ExactPointSeg),
    Version(VersionReq),
    Working(Trace),
    Pop(Trace),
}

impl FromStr for PointSegSelector {
    type Err = ParseErrs;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        result(all_consuming(point_segment_selector)(new_span(s)))
    }
}

impl PointSegSelector {
    pub fn is_exact(&self) -> bool {
        match self {
            PointSegSelector::Exact(_) => true,
            _ => false,
        }
    }

    pub fn is_match(&self, segment: &PointSeg) -> bool {
        match self {
            PointSegSelector::InclusiveAny => true,
            PointSegSelector::InclusiveRecursive => true,
            PointSegSelector::Any => true,
            PointSegSelector::Recursive => true,
            PointSegSelector::Exact(exact) => match exact {
                ExactPointSeg::PointSeg(pattern) => pattern.to_string() == segment.to_string(),
                ExactPointSeg::Version(a) => {
                    if let PointSeg::Version(b) = segment {
                        *a == *b
                    } else {
                        false
                    }
                }
            },
            PointSegSelector::Version(req) => {
                if let PointSeg::Version(b) = segment {
                    req.matches(b)
                } else {
                    false
                }
            }
        }
    }

    pub fn is_recursive(&self) -> bool {
        match self {
            PointSegSelector::InclusiveAny => false,
            PointSegSelector::InclusiveRecursive => true,
            PointSegSelector::Any => false,
            PointSegSelector::Recursive => true,
            PointSegSelector::Exact(_) => false,
            PointSegSelector::Version(_) => false,
        }
    }
}

impl ToString for PointSegSelector {
    fn to_string(&self) -> String {
        match self {
            PointSegSelector::InclusiveAny => "+:*".to_string(),
            PointSegSelector::InclusiveRecursive => "+:**".to_string(),
            PointSegSelector::Any => "*".to_string(),
            PointSegSelector::Recursive => "**".to_string(),
            PointSegSelector::Exact(exact) => exact.to_string(),
            PointSegSelector::Version(version) => version.to_string(),
        }
    }
}

pub type KeySegment = String;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub enum ExactPointSeg {
    PointSeg(PointSeg),
    Version(Version),
}

impl ExactPointSeg {
    pub fn matches(&self, segment: &PointSeg) -> bool {
        match self {
            ExactPointSeg::PointSeg(s) => *s == *segment,
            ExactPointSeg::Version(a) => {
                if let PointSeg::Version(b) = segment {
                    *a == *b
                } else {
                    false
                }
            }
        }
    }
}

impl ToString for ExactPointSeg {
    fn to_string(&self) -> String {
        match self {
            ExactPointSeg::PointSeg(point) => point.to_string(),
            ExactPointSeg::Version(version) => version.to_string(),
        }
    }
}

/// Provides ability to Select on a Specific.  This means wildcards can be applied when any match will do:
/// `mechtronhub.io:postgres.org:postgres:*:(9.0.0)` will select ANY variant of postgres version 9.0.0.
/// (notice the version MUST be delimited by Parenthesis.
/// A more useful example is when performing some type of version selection it follows SemVer Req rules:
/// `mechtronhub.io:postgres.org:postgres:gis:(>=10.2.3 <12.3.0)`
/// which would match on any version of postgres:gis with a version in that range
pub type SpecificSelector = SpecificSelectorDef<
    ProviderSelector,
    VendorSelector,
    ProductSelector,
    VariantSelector,
    VersionReq,
>;

impl SpecificSelector {
    pub fn always() -> SpecificSelector {
        Self {
            provider: ProviderSelector::Always,
            vendor: VendorSelector::Always,
            product: ProductSelector::Always,
            variant: VariantSelector::Always,
            version: VersionReq::always(),
        }
    }
}

impl FromStr for SpecificSelector {
    type Err = ParseErrs;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        result(all_consuming(specific_selector)(new_span(s)))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct SpecificSelectorDef<
    ProviderSelector,
    VendorSelector,
    ProductSelector,
    VariantSelector,
    VersionReq,
> {
    pub provider: ProviderSelector,
    pub vendor: VendorSelector,
    pub product: ProductSelector,
    pub variant: VariantSelector,
    pub version: VersionReq,
}

impl ValueMatcher<Specific> for SpecificSelector {
    fn is_match(&self, specific: &Specific) -> Result<(), ()> {
        if self.provider.is_match(&specific.provider)
            && self.vendor.is_match(&specific.vendor)
            && self.product.is_match(&specific.product)
            && self.variant.is_match(&specific.variant)
            && self.version.matches(&specific.version)
        {
            Ok(())
        } else {
            Err(())
        }
    }
}

impl ToString for SpecificSelector {
    fn to_string(&self) -> String {
        format!(
            "{}:{}:{}:({})",
            self.vendor.to_string(),
            self.product.to_string(),
            self.variant.to_string(),
            self.version.to_string()
        )
    }
}

pub mod specific {
    use core::ops::Deref;
    use core::str::FromStr;

    use crate::err::{ParseErrs, SpaceErr};
    use crate::parse::{Domain, SkewerCase};
    use crate::selector::Pattern;

    pub struct VersionReq {
        pub req: semver::VersionReq,
    }

    impl Deref for VersionReq {
        type Target = semver::VersionReq;

        fn deref(&self) -> &Self::Target {
            &self.req
        }
    }

    impl FromStr for VersionReq {
        type Err = ParseErrs;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            Ok(VersionReq {
                req: semver::VersionReq::from_str(s)?,
            })
        }
    }

    pub type ProviderSelector = Pattern<Domain>;
    pub type VendorSelector = Pattern<Domain>;
    pub type ProductSelector = Pattern<SkewerCase>;
    pub type VariantSelector = Pattern<SkewerCase>;
    pub type VersionPattern = Pattern<VersionReq>;
}

pub type LabeledPrimitiveType = LabeledPrimitiveTypeDef<Point>;
pub type LabeledPrimitiveTypeCtx = LabeledPrimitiveTypeDef<PointCtx>;
pub type LabeledPrimitiveTypeVar = LabeledPrimitiveTypeDef<PointVar>;

pub struct LabeledPrimitiveTypeDef<Pnt> {
    pub label: String,
    pub def: PayloadType2Def<Pnt>,
}

pub type PayloadType2 = PayloadType2Def<Point>;
pub type PayloadType2Ctx = PayloadType2Def<PointCtx>;
pub type PayloadType2Var = PayloadType2Def<PointVar>;

pub struct PayloadType2Def<Pnt> {
    pub primitive: SubstanceKind,
    pub format: Option<SubstanceFormat>,
    pub verifier: Option<CallWithConfigDef<Pnt>>,
}

#[derive(Debug, Clone, strum_macros::Display, strum_macros::EnumString, Eq, PartialEq)]
pub enum Format {
    #[strum(serialize = "json")]
    Json,
    #[strum(serialize = "image")]
    Image,
}

#[derive(Debug, Clone, strum_macros::Display, strum_macros::EnumString, Eq, PartialEq)]
pub enum PipelineKind {
    Rc,
    Ext,
    Http,
}

pub struct ParsedPipelineBlock {}

pub type MapEntryPatternVar = MapEntryPatternDef<PointVar>;
pub type MapEntryPatternCtx = MapEntryPatternDef<PointCtx>;
pub type MapEntryPattern = MapEntryPatternDef<Point>;

#[derive(Clone)]
pub struct MapEntryPatternDef<Pnt> {
    pub key: String,
    pub payload: ValuePattern<SubstancePatternDef<Pnt>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct HopDef<Segment, KindSelector> {
    pub inclusive: bool,
    pub segment_selector: Segment,
    pub kind_selector: KindSelector,
}

impl PointSegKindHop {
    pub fn always() -> Self {
        Self {
            inclusive: true,
            segment_selector: PointSegSelector::Any,
            kind_selector: KindSelector::any(),
        }
    }
}

impl HopDef<PointSegSelector, ValuePattern<KindSelector>> {
    pub fn is_match(&self, point_kind_segment: &PointKindSegOpt) -> bool {
        let kind = point_kind_segment.kind.clone();
        self.segment_selector.is_match(&point_kind_segment.segment)
            && self.kind_selector.is_match_opt(&kind).is_ok()
    }
}

impl ToString for PointSegKindHop {
    fn to_string(&self) -> String {
        let mut rtn = String::new();
        rtn.push_str(self.segment_selector.to_string().as_str());

        rtn.push_str(format!("<{}>", self.kind_selector.to_string()).as_str());

        /*        match &self.kind_selector {
                   ValuePattern::Always => rtn.push_str("<*>"),
                   ValuePattern::Never => rtn.push_str("<!>"),
                   ValuePattern::Pattern(kind_selector) => {
                       if let Pattern::Exact(base) = &kind_selector.base {
                           rtn.push_str(format!("<{}", base.to_string()).as_str());
                           if let Pattern::Exact(sub) = &kind_selector.sub {
                               rtn.push_str(format!("<{}", sub.as_ref().unwrap().to_string()).as_str());
                               if let ValuePattern::Pattern(specific) = &kind_selector.specific {
                                   rtn.push_str(format!("<{}>", specific.to_string()).as_str());
                               }
                               rtn.push_str(">");
                           }
                           rtn.push_str(">");
                       }
                   }
               }

        */

        if self.inclusive {
            rtn.push_str("+");
        }

        rtn
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub enum Pattern<P> {
    Always,
    Exact(P),
}

impl<I: ToString> Pattern<I> {
    pub fn to_string_version(self) -> Pattern<String> {
        match self {
            Pattern::Always => Pattern::Always,
            Pattern::Exact(exact) => Pattern::Exact(exact.to_string()),
        }
    }
}

impl<P> Pattern<P>
where
    P: Eq + PartialEq,
{
    pub fn is_any(&self) -> bool {
        match self {
            Pattern::Always => true,
            Pattern::Exact(_) => false,
        }
    }

    pub fn is_match(&self, t: &P) -> bool {
        match self {
            Self::Always => true,
            Self::Exact(p) => *p == *t,
        }
    }
    pub fn matches_opt(&self, other: Option<&P>) -> bool {
        match self {
            Self::Always => true,
            Self::Exact(exact) => {
                if let Option::Some(other) = other {
                    *exact == *other
                } else {
                    false
                }
            }
        }
    }

    pub fn convert<To>(self) -> Result<Pattern<To>, SpaceErr>
    where
        P: TryInto<To, Error = SpaceErr> + Eq + PartialEq,
    {
        Ok(match self {
            Pattern::Always => Pattern::Always,
            Pattern::Exact(exact) => Pattern::Exact(exact.try_into()?),
        })
    }
}

impl<P> ToString for Pattern<P>
where
    P: ToString,
{
    fn to_string(&self) -> String {
        match self {
            Pattern::Always => "*".to_string(),
            Pattern::Exact(exact) => exact.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EmptyPattern<P> {
    Any,
    Pattern(P),
}

impl<P> EmptyPattern<P>
where
    P: Eq + PartialEq,
{
    pub fn matches(&self, t: &P) -> bool {
        match self {
            Self::Any => true,
            Self::Pattern(p) => *p == *t,
        }
    }
    pub fn matches_opt(&self, other: Option<&P>) -> bool {
        match self {
            Self::Any => true,
            Self::Pattern(exact) => {
                if let Option::Some(other) = other {
                    *exact == *other
                } else {
                    false
                }
            }
        }
    }

    pub fn convert<To>(self) -> Result<EmptyPattern<To>, SpaceErr>
    where
        P: TryInto<To, Error = SpaceErr> + Eq + PartialEq,
    {
        Ok(match self {
            EmptyPattern::Any => EmptyPattern::Any,
            EmptyPattern::Pattern(exact) => EmptyPattern::Pattern(exact.try_into()?),
        })
    }
}

impl Into<EmptyPattern<String>> for EmptyPattern<&str> {
    fn into(self) -> EmptyPattern<String> {
        match self {
            EmptyPattern::Any => EmptyPattern::Any,
            EmptyPattern::Pattern(f) => EmptyPattern::Pattern(f.to_string()),
        }
    }
}

impl<P> ToString for EmptyPattern<P>
where
    P: ToString,
{
    fn to_string(&self) -> String {
        match self {
            EmptyPattern::Any => "".to_string(),
            EmptyPattern::Pattern(exact) => exact.to_string(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash, strum_macros::Display)]
pub enum KindBaseSelector {
    #[strum(to_string = "*")]
    Always,
    #[strum(to_string = "!")]
    Never,
    #[strum(to_string = "{0}")]
    Exact(BaseKind),
}

impl KindBaseSelector {
    pub fn is_match(&self, other: &BaseKind) -> Result<(), ()> {
        match self {
            KindBaseSelector::Always => Ok(()),
            KindBaseSelector::Never => Err(()),
            KindBaseSelector::Exact(me) if me == other => Ok(()),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct PortHierarchy {
    pub topic: Topic,
    pub layer: Layer,
    pub point_hierarchy: PointHierarchy,
}

impl PortHierarchy {
    pub fn new(point_hierarchy: PointHierarchy, layer: Layer, topic: Topic) -> Self {
        Self {
            topic,
            layer,
            point_hierarchy,
        }
    }
}

pub type PointHierarchy = PointDef<RouteSeg, PointKindSeg>;

pub type PointHierarchyOpt = PointDef<RouteSeg, PointKindSegOpt>;

impl From<&PointHierarchy> for PointHierarchyOpt {
    fn from(value: &PointHierarchy) -> Self {
        Self {
            route: value.route.clone(),
            segments: value.segments.iter().map(|s| s.clone().into()).collect(),
        }
    }
}

impl From<&Point> for PointHierarchyOpt {
    fn from(value: &Point) -> Self {
        Self {
            route: value.route.clone(),
            segments: value
                .segments
                .iter()
                .map(|s| PointKindSegOpt {
                    segment: s.clone(),
                    kind: None,
                })
                .collect(),
        }
    }
}

impl From<Point> for PointHierarchyOpt {
    fn from(value: Point) -> Self {
        Self {
            route: value.route.clone(),
            segments: value
                .segments
                .into_iter()
                .map(|segment| PointKindSegOpt {
                    segment,
                    kind: None,
                })
                .collect(),
        }
    }
}

impl FromStr for PointHierarchy {
    type Err = ParseErrs;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        consume_hierarchy(new_span(s))
    }
}

impl PointHierarchy {
    pub fn new(route: RouteSeg, segments: Vec<PointKindSeg>) -> Self {
        Self { route, segments }
    }
}

impl PointHierarchy {
    pub fn push(&self, segment: PointKindSeg) -> PointHierarchy
    where
        KindParts: Clone,
        BaseKind: Clone,
    {
        let mut segments = self.segments.clone();
        segments.push(segment);
        Self {
            route: self.route.clone(),
            segments,
        }
    }
}

impl PointDef<RouteSeg, PointKindSegOpt> {
    pub fn consume(&self) -> Option<PointHierarchyOpt> {
        if self.segments.len() <= 1 {
            return Option::None;
        }
        let mut segments = self.segments.clone();
        segments.remove(0);
        Option::Some(PointHierarchyOpt {
            route: self.route.clone(),
            segments,
        })
    }

    pub fn is_final(&self) -> bool {
        self.segments.len() == 1
    }
}

impl PointDef<RouteSeg, PointKindSeg> {
    pub fn consume(&self) -> Option<PointHierarchy> {
        if self.segments.len() <= 1 {
            return Option::None;
        }
        let mut segments = self.segments.clone();
        segments.remove(0);
        Option::Some(PointHierarchy {
            route: self.route.clone(),
            segments,
        })
    }

    pub fn is_final(&self) -> bool {
        self.segments.len() == 1
    }
}

impl Into<Point> for PointHierarchy {
    fn into(self) -> Point {
        let segments = self
            .segments
            .iter()
            .map(|seg| seg.segment.clone())
            .collect();
        Point {
            route: self.route,
            segments,
        }
    }
}

impl ToString for PointHierarchy {
    fn to_string(&self) -> String {
        let mut rtn = String::new();
        match &self.route {
            RouteSeg::This => {}
            route => {
                rtn.push_str(route.to_string().as_str());
                rtn.push_str("::");
            }
        }

        let mut post_fileroot = false;
        for (index, segment) in self.segments.iter().enumerate() {
            if let PointSeg::FsRootDir = segment.segment {
                post_fileroot = true;
            }
            rtn.push_str(segment.segment.preceding_delim(post_fileroot));
            rtn.push_str(segment.to_string().as_str());
        }

        rtn
    }
}

pub type PointKindSeg = PointKindSegDef<Kind>;
pub type PointKindSegOpt = PointKindSegDef<Option<Kind>>;

impl Into<PointKindSegOpt> for PointKindSeg {
    fn into(self) -> PointKindSegOpt {
        PointKindSegOpt {
            segment: self.segment,
            kind: Some(self.kind),
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub struct PointKindSegDef<K> {
    pub segment: PointSeg,
    pub kind: K,
}

impl ToString for PointKindSeg {
    fn to_string(&self) -> String {
        format!("{}<{}>", self.segment.to_string(), self.kind.to_string())
    }
}

impl ToString for PointKindSegOpt {
    fn to_string(&self) -> String {
        format!(
            "{}<{}>",
            self.segment.to_string(),
            match &self.kind {
                None => "?".to_string(),
                Some(kind) => kind.to_string(),
            }
        )
    }
}

pub type PayloadBlock = PayloadBlockDef<Point>;
pub type PayloadBlockCtx = PayloadBlockDef<PointCtx>;
pub type PayloadBlockVar = PayloadBlockDef<PointVar>;

impl ToResolved<PayloadBlockCtx> for PayloadBlockVar {
    fn to_resolved(self, env: &Env) -> Result<PayloadBlockCtx, ParseErrs> {
        match self {
            PayloadBlockVar::DirectPattern(block) => Ok(PayloadBlockCtx::DirectPattern(
                block.modify(move |block| {
                    let block: SubstancePatternCtx = block.to_resolved(env)?;
                    Ok(block)
                })?,
            )),
            PayloadBlockVar::ReflectPattern(block) => Ok(PayloadBlockCtx::ReflectPattern(
                block.modify(move |block| block.to_resolved(env))?,
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadBlock {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBlock {
    pub payload: Substance,
}

pub type PatternBlock = PatternBlockDef<Point>;
pub type PatternBlockCtx = PatternBlockDef<PointCtx>;
pub type PatternBlockVar = PatternBlockDef<PointVar>;
pub type PatternBlockDef<Pnt> = ValuePattern<SubstancePatternDef<Pnt>>;

impl ToResolved<PatternBlock> for PatternBlockCtx {
    fn to_resolved(self, env: &Env) -> Result<PatternBlock, ParseErrs> {
        match self {
            PatternBlockCtx::Always => Ok(PatternBlock::Always),
            PatternBlockCtx::Never => Ok(PatternBlock::Never),
            PatternBlockCtx::Pattern(pattern) => {
                Ok(PatternBlock::Pattern(pattern.to_resolved(env)?))
            }
        }
    }
}

impl ToResolved<PatternBlockCtx> for PatternBlockVar {
    fn to_resolved(self, env: &Env) -> Result<PatternBlockCtx, ParseErrs> {
        match self {
            PatternBlockVar::Always => Ok(PatternBlockCtx::Always),
            PatternBlockVar::Never => Ok(PatternBlockCtx::Never),
            PatternBlockVar::Pattern(pattern) => {
                Ok(PatternBlockCtx::Pattern(pattern.to_resolved(env)?))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum PayloadBlockDef<Pnt> {
    DirectPattern(PatternBlockDef<Pnt>),
    ReflectPattern(PatternBlockDef<Pnt>),
}

impl ToResolved<PayloadBlock> for PayloadBlockCtx {
    fn to_resolved(self, env: &Env) -> Result<PayloadBlock, ParseErrs> {
        match self {
            PayloadBlockCtx::DirectPattern(block) => {
                Ok(PayloadBlock::DirectPattern(block.modify(move |block| {
                    let block: SubstancePattern = block.to_resolved(env)?;
                    Ok(block)
                })?))
            }
            PayloadBlockCtx::ReflectPattern(block) => Ok(PayloadBlock::ReflectPattern(
                block.modify(move |block| block.to_resolved(env))?,
            )),
        }
    }
}

impl ToResolved<PayloadBlock> for PayloadBlockVar {
    fn to_resolved(self, env: &Env) -> Result<PayloadBlock, ParseErrs> {
        let block: PayloadBlockCtx = self.to_resolved(env)?;
        block.to_resolved(env)
    }
}

#[cfg(test)]
mod test {
    use crate::kind::BaseKind;
    use crate::selector::{PointKindSeg, PointSegKindHop, PointSelector};
    use crate::util::ValueMatcher;

    #[test]
    pub fn test() {
        let selector = PointSelector::always();
        let point = BaseKind::Driver.bind();
        assert!(selector.is_match(&point).is_ok())
    }

    #[test]
    pub fn segment() {
        let selector = PointSegKindHop::always();
        let point = BaseKind::Driver.bind();
        assert!(selector.is_match(point.segments.first().unwrap()).is_ok());
    }
}
