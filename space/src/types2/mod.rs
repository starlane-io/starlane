use crate::err::ParseErrs0;
use crate::parse::model::{BlockKind, LexBlock, NestedBlockKind};
use crate::parse::util::preceded;
use crate::parse::util::Span;
use crate::parse::{camel_case, CamelCase, NomErr, SkewerCase, SnakeCase};
use crate::parse::{lex_block, Res};
use crate::point::Point;
use crate::selector::Pattern;
use crate::types::class::Class;
use crate::types::data::DataType;
use crate::types::scope::Scope;
use crate::types::specific::{SpecificLoc, SpecificSelector};
use archetype::Archetype;
use derive_name::Name;
use getset::Getters;
use itertools::Itertools;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::combinator::opt;
use nom::error::{ErrorKind, FromExternalError, ParseError};
use nom::sequence::{tuple, Tuple};
use nom_supreme::context::ContextError;
use parse::Delimited;
use std::fmt::{Display, Formatter};
use std::hash::Hash;
use std::str::FromStr;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde::de::DeserializeOwned;
use strum_macros::EnumDiscriminants;
use thiserror::Error;

pub mod class;
pub mod data;

pub mod err;
pub mod registry;
pub mod specific;

pub mod def;
pub mod parse;
pub mod scope;
pub mod selector;
pub mod tag;
#[cfg(test)]
pub mod test;
pub mod archetype;
pub mod property;
pub mod package;
//pub(crate) trait Typical: Display+Into<TypeKind>+Into<Type> { }

/// [class::Class::Database] is an example of an [Type] because it is not an [ExactDef]
/// which references a definition in [SpecificLoc]
#[derive(Clone, Debug, Eq, PartialEq, Hash, EnumDiscriminants, strum_macros::Display, Serialize, Deserialize)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(TypeDisc))]
#[strum_discriminants(derive(
    Hash,
    strum_macros::EnumString,
    strum_macros::ToString,
    strum_macros::IntoStaticStr
))]
pub enum Type {
    Class(Class),
    Data(DataType),
}

impl Type {
    pub fn parse_lex_block<I>(input: I) -> Res<I, LexBlock<I>>
    where
        I: Span,
    {
        alt((
            lex_block(BlockKind::Nested(NestedBlockKind::Angle)),
            lex_block(BlockKind::Nested(NestedBlockKind::Square)),
        ))(input)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct TypeLocation<Scope, Type, Specific>
where
    Scope: Archetype + Default,
    Type: Clone,
    Specific: Clone,
{
    scope: Scope,
    r#type: Type,
    slice: Specific,
}

/// [CategoryGeneric] stands for `category` ... a [Type] is a category
/// if a [SpecificLoc] is not supplied
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct CategoryGeneric<Scope, Type>
where
    Scope: Archetype + Default,
{
    pub scope: Scope,
    pub r#type: Type,
}

impl TypeDisc {
    pub fn get_delimiters(&self) -> (&'static str, &'static str) {
        match self {
            TypeDisc::Class => ("<", ">"),
            TypeDisc::Data => ("[", "]"),
        }
    }

    pub fn parser<I>(&self) -> impl FnMut(I) -> Res<I, Type>
    where
        I: Span,
    {
        match self {
            TypeDisc::Class => |i| Class::parser(i).map(|(next, r#type)| (next, r#type.into())),
            TypeDisc::Data => |i| DataType::parser(i).map(|(next, r#type)| (next, r#type.into())),
        }
    }
}

impl From<Class> for Type {
    fn from(value: Class) -> Self {
        Type::Class(value)
    }
}

impl From<DataType> for Type {
    fn from(value: DataType) -> Self {
        Type::Data(value)
    }
}

pub type AsType = dyn Into<Absolute>;
pub type AsTypeKind = dyn Into<Type>;

pub type AbsoluteAbsoluteGeneric<Type: Archetype> = Scaffold<Scope, Type, SpecificLoc>;

pub type Absolute = Scaffold<Scope, Type, SpecificLoc>;


#[cfg(test)]
impl Absolute {
    pub fn mock_default() -> Self {
        Self::new(Default::default(),Type::Class(Class::Root),SpecificLoc::mock_default() )
    }

    pub fn mock_root() -> Self {
        let mut mock = Self::mock_default();
        mock.specific = mock.specific.root();
        mock
    }
}

impl Serialize for Absolute {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer
    {
        todo!()
    }
}



impl <'de> Deserialize<'de> for Absolute {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>
    {
        todo!()
    }
}

pub type AbsoluteSelector = Scaffold<Pattern<Scope>, Pattern<Type>, SpecificSelector>;

#[derive(Clone, Debug, Eq, PartialEq, Hash,Getters)]
#[get = "pub"]
pub struct Scaffold<Scope, T, SpecificLoc>
where
    Scope: Archetype+Default,
    SpecificLoc: Clone + Eq + PartialEq + Hash
{
    scope: Scope,
    r#type: T,
    specific: SpecificLoc,
}

impl <Scope, T, SpecificLoc> Display for Scaffold<Scope, T, SpecificLoc>
where

    Scope: Archetype+Default,
    SpecificLoc: Clone + Eq + PartialEq + Hash+ Display,
    T: Display
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}", self.r#type, self.specific)
    }
}

impl <Scope, T, SpecificLoc> Scaffold<Scope, T, SpecificLoc>
where

    Scope: Archetype+Default,
    SpecificLoc: Clone + Eq + PartialEq + Hash,
    T: Display
{
    pub fn new(scope: Scope, r#type: T, specific: SpecificLoc) -> Self {
        Self {scope,
        r#type,
            specific
        }
    }
}


#[derive(Clone)]
pub struct AbsoluteLex<Scope, Specific>
where
Scope: Archetype+Default,
Specific: Clone + Eq + PartialEq + Hash,

{
    r#absolute: Scaffold<Scope, CamelCase, Option<Specific>>,
    disc: TypeDisc,
}

impl<Scope, T, Specific> TryInto<Scaffold<Scope, T, Specific>>
    for AbsoluteLex<Scope, Specific>
where
    Scope: Archetype + Default,
    T: From<Class> + From<DataType> + Archetype,
    Specific: Archetype + for<'y> Deserialize<'y>
{
    type Error = ParseErrs0;

    fn try_into(self) -> Result<Scaffold<Scope, T, Specific>, Self::Error> {
        let r#type = match self.disc {
            TypeDisc::Class => {
                let class: Class = self.r#absolute.r#type.into();
                class.into()
            }
            TypeDisc::Data => {
                let data: DataType = self.r#absolute.r#type.into();
                data.into()
            }
        };

        Ok(Scaffold {
            scope: self.r#absolute.scope,
            r#type,
            specific: self.r#absolute.specific.ok_or(ParseErrs0::expected(
                "TryInto<Generic>",
                "Specific",
                "None",
            ))?,
        })
    }
}


impl  TryInto<Absolute>
for AbsoluteLex<Scope, SpecificLoc>
where
    Scope: Archetype + Default,
    SpecificLoc: Archetype
{
    type Error = ParseErrs0;

    fn try_into(self) -> Result<Absolute, Self::Error> {
        let r#type = match self.disc {
            TypeDisc::Class => {
                let class: Class = self.r#absolute.r#type.into();
                class.into()
            }
            TypeDisc::Data => {
                let data: DataType = self.r#absolute.r#type.into();
                data.into()
            }
        };

        Ok(Absolute::new(self.absolute.scope,r#type,self.absolute.specific.ok_or(ParseErrs0::expected("Specific", "Some", "None"))? ))
    }
}


impl<Scope, T, Specific> Into<CategoryGeneric<Scope, T>> for AbsoluteLex<Scope, Specific>
where
    Scope: Archetype + Default,
    T: From<Class> + From<DataType> + Clone,
    Specific: Archetype + Clone,
{
    fn into(self) -> CategoryGeneric<Scope, T> {
        let r#type = match self.disc {
            TypeDisc::Class => {
                let class: Class = self.r#absolute.r#type.into();
                class.into()
            }
            TypeDisc::Data => {
                let data: DataType = self.r#absolute.r#type.into();
                data.into()
            }
        };

        CategoryGeneric {
            scope: self.r#absolute.scope,
            r#type,
        }
    }
}
impl<Specific> Display for AbsoluteLex<Scope, Specific>
where
    Specific: Archetype,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.r#absolute.scope, self.r#absolute.r#type)?;
        if let Some(specific) = &self.r#absolute.specific {
            write!(f, "@{}", specific)?;
        }
        Ok(())
    }
}
impl<Scope, Specific> AbsoluteLex<Scope, Specific>
where
    Scope: Archetype + Default,
    Specific: Archetype,
{
    fn outer_parser<I>(input: I) -> Res<I, Self>
    where
        I: Span,
    {
        let (next, block) = Type::parse_lex_block(input.clone())?;

        let disc = match block.kind {
            BlockKind::Nested(NestedBlockKind::Angle) => TypeDisc::Class,
            BlockKind::Nested(NestedBlockKind::Square) => TypeDisc::Data,
            kind => {
                let tree = nom::Err::Error(NomErr::from_error_kind(input, ErrorKind::Fail));
                return Err(tree);
            }
        };

        tuple((
            opt(Scope::parser),
            camel_case,
            opt(preceded(tag("@"), Specific::parser)),
        ))(block.content)
        .map(|(_, (scope, r#type, specific))| {
            let scope = scope.unwrap_or_default();
            let generic = Scaffold {
                scope,
                r#type,
                specific,
            };

            let lex = Self { r#absolute: generic, disc };

            (next, lex)
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Error)]
pub enum CastErr {
    #[error("a `Cat` (category) cannot be turned into a `Generic` that requires a `Specific`")]
    MissingSpecific,
}

/*
impl<Scope, Type, Specific> TryInto<Generic<Scope, Type, Specific>>
    for GenericLex<Scope, Specific>
where
    Scope: Parsable + Default,
    Specific: Parsable,
    Type: From<CamelCase> + Clone,
{
    type Error = CastErr;

    fn try_into(self) -> Result<Generic<Scope, Type, Specific>, Self::Error> {
        if let Some(specific) = self.generic.specific {
            Ok(Generic {
                scope: self.generic.scope,
                r#type: self.generic.r#type.into(),
                specific,
            })
        } else {
            Err(CastErr::MissingSpecific)
        }
    }
}

 */

/*
impl<Scope, Type, Specific> Into<CategoryGeneric<Scope, Type>> for GenericLex<Scope, Specific>
where
    Scope: Parsable + Default,
    Type: From<CamelCase> + Clone,
    Specific: Parsable,
{
    fn into(self) -> CategoryGeneric<Scope, Type> {
        CategoryGeneric {
            scope: self.generic.scope,
            r#type: self.generic.r#type.into(),
        }
    }
}

 */

impl<Scope, Specific> Scaffold<Scope, Class, Specific>
where
    Scope: Archetype + Default,
    Specific: Archetype,
{
    fn abstract_disc(&self) -> &'static TypeDisc {
        &TypeDisc::Class
    }
}

impl<Scope, Specific> Scaffold<Scope, DataType, Specific>
where
    Scope: Archetype + Default,
    Specific: Archetype,
{
    fn abstract_disc(&self) -> &'static TypeDisc {
        &TypeDisc::Data
    }
}

/*
impl <Scope,Type,Specific> ExactGen<Scope,Type,Specific> {
    pub fn new( scope: Scope, r#type: Type, specific: Specific ) -> ExactGen<Scope,Type,Specific>{
        Self {scope, r#type, specific}
    }
}

 */

impl Type {
    pub fn convention(&self) -> Convention {
        /// it so happens everything is CamelCase, but that may change...
        Convention::CamelCase
    }
}

pub enum Convention {
    CamelCase,
    SkewerCase,
}

impl Convention {
    pub fn validate(&self, text: &str) -> Result<(), ParseErrs0> {
        /// transform from [Result<Whatever, ParseErrs0>] -> [Result<(),ParseErrs?]
        fn strip_ok<Ok, Err>(result: Result<Ok, Err>) -> Result<(), Err> {
            result.map(|_| ())
        }

        match self {
            Convention::CamelCase => strip_ok(CamelCase::from_str(text)),

            Convention::SkewerCase => strip_ok(SkewerCase::from_str(text)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct PointTypeDef<Point, Type> {
    point: Point,
    r#type: Type,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct SrcDef<Point, Kind> {
    kind: Kind,
    point: Point,
}

pub type PointKindDefSrc<Kind> = SrcDef<Point, Kind>;

pub type DataPoint = PointTypeDef<Point, DataType>;

/// meaning where does this Type definition come from
/// * [DefSrc::Builtin] indicates a definition native to Starlane
/// * [DefSrc::Ext] indicates a definition extension defined outside of native Starlane
///                 potentially installed by a package
pub enum DefSrc {
    Builtin,
    Ext,
}

/// tag identifier [Tag::id] and `type`
pub struct Tag<T> {
    id: SkewerCase,
    r#type: T,
}

/// wraps a generic `segment` with a potential [Tag<T>]
pub enum TagWrap<S, T> {
    Tag(Tag<T>),
    Segment(S),
}



pub type ClassPointRef = Ref<Point, Class>;
pub type SchemaPointRef = Ref<Point, DataType>;
pub type ParsePointRef<G: Archetype> = Ref<Point, G>;
pub type ExactPointRef = Ref<Point, Absolute>;

#[derive(Clone, Eq, PartialEq, Hash)]
pub struct Ref<I, K>
where
    I: Clone + Eq + PartialEq + Hash,
    K: Clone + Eq + PartialEq + Hash,
{
    id: I,
    r#type: K,
}

impl<Scope, Type, Specific> Scaffold<Scope, Type, Specific>
where
    Scope: Archetype + Default,
    Type: Delimited,
    Specific: Archetype,
{
    pub fn of(r#type: Type, specific: Specific) -> Self {
        Self::new(Scope::default(), r#type, specific)
    }
}

pub type PropertyName = SnakeCase;


#[cfg(test)]
pub mod test2 {
    use crate::parse::util::{new_span, result};
    use crate::parse::SkewerCase;
    use crate::types::archetype::Archetype;
    use crate::types::class::Class;
    use crate::types::scope::parse::scope;
    use crate::types::scope::{Scope, ScopeKeyword, Segment};
    use crate::types::specific::SpecificLoc;
    use crate::types::{Absolute, AbsoluteLex, CategoryGeneric, Type};
    use nom::Parser;
    use starlane_space::types::data::DataType;
    use std::str::FromStr;

    #[test]
    pub fn test_specific() {
        let specific = result(SpecificLoc::parser(new_span("contrib:package:1.0.0"))).unwrap();

        assert_eq!("contrib", specific.contributor().as_str());
        assert_eq!("package", specific.package().as_str());
        assert_eq!("1.0.0", specific.version().clone().to_string().as_str());
        assert!(specific.slices().is_empty())
    }


    #[test]
    pub fn test_specific_slice_segments() {
        let specific = result(SpecificLoc::parser(new_span("contrib:package:1.0.0::slice"))).unwrap();

        assert_eq!(1,specific.slices().len());
        assert_eq!("slice", specific.slices().first().unwrap().clone().to_string().as_str());


        let specific = result(SpecificLoc::parser(new_span("contrib:package:1.0.0::one:two"))).unwrap();

        assert_eq!(2,specific.slices().len());
        let segments = specific.slices().clone();
        let mut i = segments.iter();
        assert_eq!("one", i.next().unwrap().clone().to_string().as_str());
        assert_eq!("two", i.next().unwrap().clone().to_string().as_str());
        
        /// test [Segment::Version]
        let specific = result(SpecificLoc::parser(new_span("contrib:package:1.0.0::1.2.3:two"))).unwrap();
        assert_eq!(2,specific.slices().len());

        let segments = specific.slices().clone();
        let mut i = segments.iter();
        assert_eq!("1.2.3", i.next().unwrap().clone().to_string().as_str());
        assert_eq!("two", i.next().unwrap().clone().to_string().as_str());
    }

    #[test]
    pub fn test_abstract() {
        let i = new_span("<File>");
        let lex: AbsoluteLex<Scope, SpecificLoc> = AbsoluteLex::outer_parser(i).unwrap().1;
        let cat: CategoryGeneric<Scope, Type> = lex.try_into().unwrap();
        assert_eq!(cat.r#type, Type::Class(Class::File));

        let i = new_span("[BindConfig]");
        let lex: AbsoluteLex<Scope, SpecificLoc> = AbsoluteLex::outer_parser(i).unwrap().1;
        let cat: CategoryGeneric<Scope, Type> = lex.try_into().unwrap();
        assert_eq!(cat.r#type, Type::Data(DataType::BindConfig));
    }

    #[test]
    pub fn test_full() {
        let i = new_span("<File@contrib:package:1.0.0>");
        let lex: AbsoluteLex<Scope, SpecificLoc> = AbsoluteLex::outer_parser(i).unwrap().1;
        let r#absolute: Absolute = lex.try_into().unwrap();
        assert_eq!(r#absolute.r#type, Type::Class(Class::File));
        assert_eq!(r#absolute.scope, Scope::default());
        assert_eq!(r#absolute.specific.to_string().as_str(), "contrib:package:1.0.0");

        let i = new_span("[BindConfig@contrib:package:1.0.0]");
        let lex: AbsoluteLex<Scope, SpecificLoc> = AbsoluteLex::outer_parser(i).unwrap().1;
        let full: Absolute = lex.try_into().unwrap();
        assert_eq!(full.r#type, Type::Data(DataType::BindConfig));
        assert_eq!(full.scope, Scope::default());
        assert_eq!(full.specific.to_string().as_str(), "contrib:package:1.0.0");
    }

    #[test]
    pub fn test_full_scope() {
        let i = new_span("<my::File@contrib:package:1.0.0>");
        let lex: AbsoluteLex<Scope, SpecificLoc> = AbsoluteLex::outer_parser(i).unwrap().1;
        let full: Absolute = lex.try_into().unwrap();
        assert_eq!(full.r#type, Type::Class(Class::File));
        assert_eq!(full.scope.to_string().as_str(), "my");
        assert_eq!(full.specific.to_string().as_str(), "contrib:package:1.0.0");

        let i = new_span("[my::BindConfig@contrib:package:1.0.0]");
        let lex: AbsoluteLex<Scope, SpecificLoc> = AbsoluteLex::outer_parser(i).unwrap().1;
        let full: Absolute = lex.try_into().unwrap();
        assert_eq!(full.r#type, Type::Data(DataType::BindConfig));
        assert_eq!(full.scope.to_string().as_str(), "my");
        assert_eq!(full.specific.to_string().as_str(), "contrib:package:1.0.0");
    }

    #[test]
    fn test_scope() {
        assert_eq!(
            scope(new_span("root::")).unwrap().1,
            Scope::new(Some(ScopeKeyword::Root), vec![])
        );
        assert_eq!(
            scope(new_span("my::")).unwrap().1,
            Scope::new(
                None,
                vec![Segment::Segment(SkewerCase::from_str("my").unwrap())]
            )
        );
        assert_eq!(
            scope(new_span("my::Root")).unwrap().1,
            Scope::new(
                None,
                vec![Segment::Segment(SkewerCase::from_str("my").unwrap())]
            )
        );
        assert_eq!(
            scope(new_span("my::more::Root")).unwrap().1,
            Scope::new(
                None,
                vec![
                    Segment::Segment(SkewerCase::from_str("my").unwrap()),
                    Segment::Segment(SkewerCase::from_str("more").unwrap())
                ]
            )
        );
        assert_eq!(
            scope(new_span("root::more::Root")).unwrap().1,
            Scope::new(
                Some(ScopeKeyword::Root),
                vec![Segment::Segment(SkewerCase::from_str("more").unwrap())]
            )
        );
        assert_eq!(scope(new_span("Root")).is_err(), true);
    }

    #[test]
    pub fn id_abstract_disc() {
        let i = new_span("<File>");
        let lex: AbsoluteLex<Scope, SpecificLoc> = AbsoluteLex::outer_parser(i).unwrap().1;
        let cat: CategoryGeneric<Scope, Type> = lex.try_into().unwrap();
        assert_eq!(cat.r#type, Type::Class(Class::File));
    }
}
