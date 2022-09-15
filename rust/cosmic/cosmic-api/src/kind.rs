use crate::id::id::Version;
use crate::parse::{CamelCase, Domain, SkewerCase};
use http::uri::Parts;
use serde::{Deserialize, Serialize};
use strum::ParseError::VariantNotFound;
use crate::selector::selector::VersionReq;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct SubTypeDef<Part, SubType> {
    pub part: Part,
    pub sub: SubType,
    pub r#type: SubType,
}

impl<Part, SubType, IsMatchPart, IsMatchSubType> IsMatch<SubTypeDef<Part, SubType>>
    for SubTypeDef<IsMatchPart, IsMatchSubType>
where
    IsMatchPart: IsMatch<Part>,
    IsMatchSubType: IsMatch<SubType>,
    Part: Eq + PartialEq,
    SubType: Eq + PartialEq,
{
    fn is_match(&self, other: &SubTypeDef<Part, SubType>) -> bool {
        self.part.is_match(&other.part)
            && self.sub.is_match(&other.sub)
            && self.r#type.is_match(&other.r#type)
    }
}

impl<Part, SubType> SubTypeDef<Part, SubType> {
    pub fn with_sub(self, sub: SubType) -> Self {
        Self {
            part: self.part,
            r#type: self.r#type,
            sub,
        }
    }

    pub fn with_type(self, r#type: SubType) -> Self {
        Self {
            part: self.part,
            sub: self.sub,
            r#type,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct ParentChildDef<Parent, Child> {
    pub parent: Parent,
    pub child: Child,
}

impl<Parent,Child,IsMatchParent,IsMatchChild> IsMatch<ParentChildDef<Parent,Child>> for ParentChildDef<IsMatchParent,IsMatchChild> where IsMatchParent: IsMatch<Parent>, IsMatchChild: IsMatch<Child>, Parent: Eq+PartialEq, Child: Eq+PartialEq {
    fn is_match(&self, other: &ParentChildDef<Parent, Child>) -> bool {
        self.parent.is_match(&other.parent) && self.child.is_match(&other.child )
    }
}

impl<Parent, Child> Default for ParentChildDef<Parent, Child>
where
    Parent: Default,
    Child: Default,
{
    fn default() -> Self {
        Self {
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash, strum_macros::Display)]
pub enum Variant {
    Artifact,
    Db(Db),
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
pub enum Db {
    Rel,
}

impl Variant {
    pub fn to_sub_types(self) -> VariantSubTypes {
        VariantSubTypes {
            part: self,
            sub: None,
            r#type: None,
        }
    }

    pub fn with_specific(self, specific: Option<SpecificFull>) -> VariantFull {
        VariantFull {
            parent: self.to_sub_types(),
            child: specific,
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
pub enum Kind {
    Root,
    Space,
    Auth,
    Base,
    Mechtron,
    FileSys,
    Db,
    Artifact,
    Control,
    Portal,
    Star,
    Driver,
    Global,
}

impl Kind {
    pub fn to_sub_types(self) -> KindSubTypes {
        KindSubTypes {
            part: self,
            sub: None,
            r#type: None,
        }
    }

    pub fn with_variant(self, variant: Option<VariantFull>) -> KindFull {
        KindFull {
            parent: self.to_sub_types(),
            child: variant,
        }
    }
}

impl Default for Kind {
    fn default() -> Self {
        Self::Root
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub struct SpecificDef<Domain, Skewer, Version> {
    pub provider: Domain,
    pub vendor: Domain,
    pub product: Skewer,
    pub variant: Skewer,
    pub version: Version,
}

pub type Specific = SpecificDef<Domain, SkewerCase, Version>;

impl Specific {
    pub fn new(
        provider: Domain,
        vendor: Domain,
        product: SkewerCase,
        variant: SkewerCase,
        version: Version,
    ) -> Self {
        Self {
            provider,
            vendor,
            product,
            variant,
            version,
        }
    }

    pub fn to_full(self) -> SpecificFull {
        SpecificFull {
            part: self,
            sub: None,
            r#type: None,
        }
    }

    pub fn sub(self, sub: Option<CamelCase>) -> SpecificFull {
        SpecificFull {
            part: self,
            sub,
            r#type: None,
        }
    }

    pub fn sub_type(self, sub: Option<CamelCase>, r#type: Option<CamelCase>) -> SpecificFull {
        SpecificFull {
            part: self,
            sub,
            r#type,
        }
    }
}

pub type SpecificFull = MatcherDef<Specific, Option<CamelCase>>;
pub type VariantSubTypes = MatcherDef<Variant, Option<CamelCase>>;
pub type VariantFull = ParentMatcherDef<Variant, Option<SpecificFull>, Option<CamelCase>>;
pub type KindSubTypes = MatcherDef<Kind, Option<CamelCase>>;
pub type KindFull = ParentMatcherDef<Kind, Option<VariantFull>, Option<CamelCase>>;

pub type MatcherDef<Matcher, SubTypeMatcher> = SubTypeDef<Matcher, SubTypeMatcher>;
pub type ParentMatcherDef<Matcher, Child, SubTypeMatcher> =
    ParentChildDef<SubTypeDef<Matcher, SubTypeMatcher>, Child>;

pub trait IsMatch<X>
where
    X: Eq + PartialEq,
{
    fn is_match(&self, other: &X) -> bool;
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum Pattern<X> {
    None,
    Any,
    Matches(X),
}

impl<X> IsMatch<X> for Pattern<X>
where
    X: Eq + PartialEq,
{
    fn is_match(&self, other: &X) -> bool {
        match self {
            Pattern::None => false,
            Pattern::Any => true,
            Pattern::Matches(x) => x.eq(other),
        }
    }
}

impl<X> ToString for Pattern<X>
where
    X: ToString,
{
    fn to_string(&self) -> String {
        match self {
            Pattern::None => "!".to_string(),
            Pattern::Any => "*".to_string(),
            Pattern::Matches(x) => x.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum OptPattern<X> {
    None,
    Any,
    Matches(X),
}

impl<X> IsMatch<Option<X>> for OptPattern<X>
    where
        X: Eq + PartialEq,
{
    fn is_match(&self, other: &Option<X>) -> bool {
        match self {
            Self::None => other.is_none(),
            Self::Any => true,
            Self::Matches(x) => match other {
                None => false,
                Some(o) => {
                    *x == *o
                }
            },
        }
    }
}

impl<X> ToString for OptPattern<X>
    where
        X: ToString,
{
    fn to_string(&self) -> String {
        match self {
            Self::None => "!".to_string(),
            Self::Any => "*".to_string(),
            Self::Matches(x) => x.to_string(),
        }
    }
}


impl IsMatch<Version> for VersionReq {
    fn is_match(&self, other: &Version) -> bool {
        self.version.matches(&other.version)
    }
}

pub type DomainSelector = Pattern<Domain>;
pub type SkewerSelector = Pattern<SkewerCase>;
pub type VersionSelector = Pattern<VersionReq>;
pub type SpecificSelector = SpecificDef<DomainSelector,SkewerSelector,VersionSelector>;
pub type SpecificFullSelector = MatcherDef<SpecificSelector, OptPattern<CamelCase>>;

impl SpecificSelector {
    pub fn to_full(self) -> SpecificFullSelector {
        SpecificFullSelector {
            part: self,
            sub: OptPattern::None,
            r#type: OptPattern::None
        }
    }

}

impl IsMatch<Specific> for SpecificSelector {
    fn is_match(&self, other: &Specific) -> bool {
        self.provider.is_match(&other.provider) &&
        self.vendor.is_match(&other.vendor) &&
            self.product.is_match(  &other.product ) &&
            self.variant.is_match(&other.variant )
    }
}


#[cfg(test)]
pub mod test {
    use crate::id::id::Version;
    use crate::kind::{DomainSelector, IsMatch, Kind, OptPattern, SkewerSelector, Specific, SpecificFull, SpecificSelector, Variant, VariantFull, VersionSelector};
    use crate::parse::{CamelCase, Domain, SkewerCase};
    use core::str::FromStr;
    use crate::selector::selector::VersionReq;

    fn create_specific() -> Specific {
        Specific::new(
            Domain::from_str("my-domain.com").unwrap(),
            Domain::from_str("my-domain.com").unwrap(),
            SkewerCase::from_str("product").unwrap(),
            SkewerCase::from_str("variant").unwrap(),
            Version::from_str("1.0.0").unwrap(),
        )
    }

    fn create_specific_sub_type() -> SpecificFull {
        create_specific().sub(Some(CamelCase::from_str("Blah").unwrap()))
    }

    fn create_variant_full() -> VariantFull {
        Variant::Artifact.with_specific(Some(create_specific_sub_type()))
    }

    #[test]
    pub fn specific() {
        let specific1 = create_specific();
        let specific2 = create_specific();
        assert_eq!(specific1, specific2);

        let spec1 = create_specific_sub_type();
        let spec2 = create_specific_sub_type();
        assert_eq!(spec1, spec2);
    }

    #[test]
    pub fn variant() {
        let var1 = Variant::Artifact.with_specific(Some(create_specific_sub_type()));
        let var2 = Variant::Artifact.with_specific(Some(create_specific_sub_type()));
        assert_eq!(var1, var2);
    }

    #[test]
    pub fn kind() {
        let kind1 = Kind::Root.with_variant(Some(create_variant_full()));
        let kind2 = Kind::Root.with_variant(Some(create_variant_full()));
        assert_eq!(kind1, kind2);
    }

    #[test]
    pub fn specific_selector() {
        let specific = create_specific();
        let selector = SpecificSelector {
            provider: DomainSelector::Any,
            vendor: DomainSelector::Matches(Domain::from_str("my-domain.com").unwrap()),
            product: SkewerSelector::Any,
            variant: SkewerSelector::Matches(SkewerCase::from_str("variant").unwrap()),
            version: VersionSelector::Matches(VersionReq::from_str("^1.0.0").unwrap())
        };

        assert!(selector.is_match(&specific));

        let mut specific = specific.to_full();
        let mut selector = selector.to_full();

        assert!(selector.is_match(&specific));

        let specific = specific.with_sub(Some(CamelCase::from_str("Zophis").unwrap()));
        assert!(!selector.is_match(&specific));
        let selector = selector.with_sub(OptPattern::Any);
        assert!(selector.is_match(&specific));

        let selector = SpecificSelector {
            provider: DomainSelector::None,
            vendor: DomainSelector::Matches(Domain::from_str("my-domain.com").unwrap()),
            product: SkewerSelector::Any,
            variant: SkewerSelector::Matches(SkewerCase::from_str("variant").unwrap()),
            version: VersionSelector::Matches(VersionReq::from_str("^1.0.0").unwrap())
        };

        let specific = create_specific();

        assert!(!selector.is_match(&specific));

    }
}
