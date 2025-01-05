use crate::parse::SkewerCase;
use crate::point;

#[non_exhaustive]
pub enum VersionTag{
    /// reference the latest version...
    Latest,
    /// custom [VersionTag] defined in the registry
    _Ext(SkewerCase)
}

#[non_exhaustive]
pub enum RouteTag{
    /// references the default hub `hub.starlane.io` by default
    Hub,
}

pub enum PointTag {
    _Ext(point::PointSeg),
}