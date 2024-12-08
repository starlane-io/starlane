use crate::types::{private, TypeCategory, SrcDef, TypeKind, Type};
use core::str::FromStr;
use strum_macros::EnumDiscriminants;
use starlane_space::kind::Specific;
use starlane_space::types::PointKindDefSrc;
use crate::parse::{CamelCase, SkewerCase};



/// a segment providing `scope` [Specific] [Meta] in the case where
/// multiple definitions of the same base type and/or to group like definitions
/// together.
///
///
/// Example for a [super::class::Class::File]
///
///
#[derive(Clone, Debug, Eq, PartialEq, Hash, EnumDiscriminants, strum_macros::Display)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(DomainScopeSegmentKind))]
#[strum_discriminants(derive(
    Clone,
    Debug,
    Hash,
    strum_macros::EnumString,
    strum_macros::ToString,
    strum_macros::IntoStaticStr
))]
pub enum DomainScopeSegment {
    #[strum(to_string = "{0}")]
    _Ext(SkewerCase),
}


impl FromStr for DomainScopeSegment{
    type Err = eyre::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        fn ext(s: &str) -> Result<DomainScopeSegment, eyre::Error> {
            Ok(DomainScopeSegment::_Ext(SkewerCase::from_str(s)?.into()))
        }

        match DomainScopeSegmentKind::from_str(s) {
            /// this Ok match is actually an Error!
            Ok(DomainScopeSegmentKind::_Ext) => ext(s),

//            Ok(kind) => ext(kind.into()),
            Err(_) => ext(s),
        }
    }
}


