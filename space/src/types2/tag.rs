use crate::parse::SkewerCase;
use crate::point;
use crate::types::specific::SpecificLoc;

#[non_exhaustive]
pub enum VersionTag {
    /// magically derive the version in this order:
    /// 1. [VersionTag::Using] (if set)
    /// 2. [VersionTag::Latest] use the latest
    Default,
    /// the global version number for [SpecificLoc]
    Using,
    /// reference the latest version...
    Latest,

    /// custom [VersionTag] defined in the registry
    _Ext(SkewerCase),
}

#[non_exhaustive]
pub enum RouteTag {
    /// references the default hub `hub.starlane.io` by default
    Hub,
}

#[non_exhaustive]
pub enum PointTag {
    _Ext(point::PointSeg),
}
