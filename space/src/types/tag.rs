use std::fmt::Display;
use std::hash::Hash;
use std::str::FromStr;
use derive_name::Name;
use serde_derive::{Deserialize, Serialize};
use strum::ParseError;
use strum_macros::EnumDiscriminants;
use starlane_space::types::tag::parse::wrap_tag;
use crate::parse::{Res, SkewerCase};
use crate::parse::util::Span;
use crate::point;
use crate::types::specific::Specific;
#[derive(Clone, Eq,PartialEq,Hash,Debug, EnumDiscriminants, strum_macros::Display, Serialize, Deserialize,Name )]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(TagDiscriminant))]
#[strum_discriminants(derive(
    Hash,
    strum_macros::EnumString,
    strum_macros::ToString,
    strum_macros::IntoStaticStr
))]
#[non_exhaustive]
#[strum(serialize_all = "kebab-case")]
pub enum Tag {
    Version(VersionTag),
    Route(RouteTag),
    Point(PointTag),
    Specific(SpecificTag)
}

pub trait AbstractTag: Display+Eq+PartialEq+Hash+Into<Tag>+From<SkewerCase> {
    fn parse<I>(input:I) -> Res<I,Self> where I:Span{
        parse::any_tag(input)
    }

    fn wrap<I,F,O>(f: F) -> impl FnMut(I) -> Res<I,TagWrap<O,Self>> where F: FnMut(I) -> Res<I,O>+Copy, I: Span, O: Display  {
        wrap_tag(f)
    }


    fn to_wrapped_string(&self) -> String {
        format!("#[{}]", self).to_string()
    }

}

impl From<VersionTag> for Tag {
    fn from(tag: VersionTag) -> Self {
        Tag::Version(tag)
    }
}

impl From<RouteTag> for Tag {
    fn from(tag: RouteTag) -> Self {
        Tag::Route(tag)
    }
}

impl From<PointTag> for Tag {
    fn from(tag: PointTag) -> Self {
        Tag::Point(tag)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TagWrap<S,T> where T: AbstractTag+Display, S: Display {
    Segment(S),
    Tag(T)
}


impl <S,T> Display for TagWrap<S,T> where T: AbstractTag+Display, S: Display {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            TagWrap::Segment(segment) => segment.to_string(),
            TagWrap::Tag(tag) => tag.to_wrapped_string()
        };
        write!(f, "{}", str)
    }
}
impl <S,T> TagWrap<S,T> where T: AbstractTag+Display, S: Display {
    pub fn segment(segment:S) -> Self {
        Self::Segment(segment)
    }

    pub fn tag(tag:T) -> Self {
        Self::Tag(tag)
    }
}




#[derive(Clone, Eq,PartialEq,Hash,Debug, EnumDiscriminants, strum_macros::Display, Serialize, Deserialize,Name, strum_macros::EnumString )]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(VersionTagDiscriminant))]
#[strum_discriminants(derive(
    Hash,
    strum_macros::EnumString,
    strum_macros::ToString,
    strum_macros::IntoStaticStr
))]
#[non_exhaustive]
#[strum(serialize_all = "kebab-case")]
pub enum VersionTag{
    /// magically derive the version in this order:
    /// 1. [VersionTag::Using] (if set)
    /// 2. [VersionTag::Latest] use the latest
    Default,
    /// the global version number for [Specific]
    Using,
    /// reference the latest version...
    Latest,

    /// custom [VersionTag] defined in the registry
    #[strum(disabled)]
    #[strum(to_string = "{0}")]
    _Ext(SkewerCase)
}

impl AbstractTag for VersionTag {


}



impl From<SkewerCase> for VersionTag {
    fn from(skewer: SkewerCase) -> Self {
        match VersionTag::from_str(skewer.as_str()) {
            Ok(tag) => tag,
            Err(_) => VersionTag::_Ext(skewer)
        }
    }
}



#[derive(Clone, Eq,PartialEq,Hash,Debug, EnumDiscriminants, strum_macros::Display, Serialize, Deserialize,Name, strum_macros::EnumString )]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(RouteTagDiscriminants))]
#[strum_discriminants(derive(
    Hash,
    strum_macros::EnumString,
    strum_macros::ToString,
    strum_macros::IntoStaticStr
))]
#[non_exhaustive]
#[strum(serialize_all = "kebab-case")]
pub enum RouteTag{
    /// references the default hub `hub.starlane.io` by default
    Hub,
    #[strum(disabled)]
    #[strum(to_string = "{0}")]
    _Ext(SkewerCase)
}

impl AbstractTag for RouteTag {}

impl From<SkewerCase> for RouteTag {
    fn from(skewer: SkewerCase) -> Self {
        match Self::from_str(skewer.as_str()) {
            Ok(tag) => tag,
            Err(_) => Self::_Ext(skewer)
        }
    }
}


#[derive(Clone, Eq,PartialEq,Hash,Debug, EnumDiscriminants, strum_macros::Display, Serialize, Deserialize,Name, strum_macros::EnumString )]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(SpecificTagdiscriminants))]
#[strum_discriminants(derive(
    Hash,
    strum_macros::EnumString,
    strum_macros::ToString,
    strum_macros::IntoStaticStr
))]
#[non_exhaustive]
#[strum(serialize_all = "kebab-case")]
pub enum SpecificTag{
    #[strum(disabled)]
    #[strum(to_string = "{0}")]
    _Ext(SkewerCase),
}


impl Into<Tag> for SpecificTag {
    fn into(self) -> Tag {
        Tag::Specific(self)
    }
}

impl AbstractTag for SpecificTag {}

impl From<SkewerCase> for SpecificTag {
    fn from(skewer: SkewerCase) -> Self {
        match Self::from_str(skewer.as_str()) {
            Ok(tag) => tag,
            Err(_) => Self::_Ext(skewer)
        }
    }
}
#[derive(Clone, Eq,PartialEq,Hash,Debug, EnumDiscriminants, strum_macros::Display, Serialize, Deserialize,Name, strum_macros::EnumString )]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(PointTagDiscriminants))]
#[strum_discriminants(derive(
    Hash,
    strum_macros::EnumString,
    strum_macros::ToString,
    strum_macros::IntoStaticStr
))]
#[non_exhaustive]
#[strum(serialize_all = "kebab-case")]
pub enum PointTag {
    #[strum(disabled)]
    #[strum(to_string = "{0}")]
    _Ext(SkewerCase),
}

impl AbstractTag for crate::types::tag::PointTag {}

impl From<SkewerCase> for crate::types::tag::PointTag {
    fn from(skewer: SkewerCase) -> Self {
        match Self::from_str(skewer.as_str()) {
            Ok(tag) => tag,
            Err(_) => Self::_Ext(skewer)
        }
    }
}



pub mod parse {
    use nom::combinator::into;
    use nom::sequence::delimited;
    use nom_supreme::tag::complete::tag;
    use crate::parse::{skewer_case, Res, SkewerCase};
    use crate::parse::util::Span;
    use crate::types::tag::{AbstractTag, VersionTag};

    pub use wrap::wrapper as wrap_tag;

    mod wrap{
        use std::fmt::Display;
        use nom::branch::alt;
        use nom::{InputLength, Parser};
        use crate::parse::{Res, SkewerCase};
        use crate::parse::util::Span;
        use crate::types::tag::{AbstractTag, TagWrap};
        use super::tag_block;
        pub fn wrapper<I,F,T,S>(f: F) -> impl FnMut(I) -> Res<I,TagWrap<S,T>> where F: FnMut(I) -> Res<I, S>+Copy, I:Span, T: AbstractTag, S: Display{
            alt((with_tag,with_seg(f)))
        }

        fn with_tag<I,S,T>( input: I) -> Res<I,TagWrap<S,T>>  where I: Span, T: AbstractTag, S: Display {
            tag_block(input).map(|(next,tag)|(next,TagWrap::Tag(tag)))
        }

        fn with_seg<I,F,T,S>(mut f: F) -> impl FnMut(I) -> Res<I,TagWrap<S,T>> where F: FnMut(I) -> Res<I, S>+Copy, I:Span, T: AbstractTag, S: Display {
            move |input| f(input).map(|(next,segment)|(next,TagWrap::Segment(segment)))
        }
    }


    pub fn tag_block<I,T>(input: I) -> Res<I,T> where T: AbstractTag, I: Span {
        delimited(tag("#["),any_tag,tag("]"))(input)
    }

    pub fn any_tag<I,T>(input: I) -> Res<I,T>  where I: Span, T: AbstractTag {
        into(skewer_case)(input)
    }


    #[cfg(test)]
    pub mod test {
        use std::str::FromStr;
        use nom::Finish;
        use crate::parse::{skewer_case, SkewerCase};
        use crate::parse::util::{new_span, result};
        use crate::types::tag::parse::wrap_tag;
        use crate::types::tag::{AbstractTag, TagWrap, VersionTag};

        #[test]
        pub fn test_version_tag() {
            assert_eq!(VersionTag::Default,result(VersionTag::parse(new_span("default"))).unwrap());

            assert_eq!(VersionTag::Using,result(VersionTag::parse(new_span("using"))).unwrap());
            assert_eq!(VersionTag::Latest,result(VersionTag::parse(new_span("latest"))).unwrap());
            assert_eq!(VersionTag::_Ext(SkewerCase::from_str("my-tag").unwrap()).to_string().as_str(),result(VersionTag::parse(new_span("my-tag"))).unwrap().to_string().as_str());
        }


        #[test]
        pub fn test_wrap() {
            let inner = "default";
            let outer = format!("#[{}]", inner);
            let input = new_span(outer.as_str());
            let wrapper = result(VersionTag::wrap(skewer_case)(input)).unwrap();

            assert_eq!(TagWrap::tag(VersionTag::Default),wrapper);
        }
    }

}