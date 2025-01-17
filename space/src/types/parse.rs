use std::fmt::Display;
use std::marker::PhantomData;
use crate::parse::util::{preceded, Span};
use crate::parse::{CamelCase, Res};
use crate::types::variant::class::Class;
use crate::types::{specific, Ext, Schema, Type, TypeDiscriminant, TypeVariant};
use futures::FutureExt;
use nom::combinator::{into, opt, peek};
use nom::sequence::{pair, tuple};
use nom::Parser;
use nom_supreme::tag::complete::tag;
use nom_supreme::ParserExt;
use starlane_space::parse::{from_camel, parse_from_str};
use std::str::FromStr;
use nom::error::FromExternalError;
use nom::multi::many0;
use once_cell::sync::Lazy;
use starlane_space::types::ExtVariant;
use starlane_space::types::specific::SpecificVariant;
use crate::parse::model::NestedBlockKind;
use crate::types::parse::util::TypeVariantStack;
use crate::types::specific::SpecificExt;
use crate::types::variant::Identifier;

pub static NESTED_BLOCKS_DEFAULT: Lazy<Option<NestedBlockKind>> =
    Lazy::new(|| None);

/// every 'type' needs to support [PrimitiveArchetype] traits
pub trait PrimitiveArchetype: Display {
    type Parser: ?Sized;
}


pub trait PrimitiveParser {
    type Output;

    fn peek<I>(input: I) -> Res<I,Self::Output> where I: Span {
        peek(Self::parse)(input)
    }

    fn parse<I>(input: I) -> Res<I,Self::Output> where I: Span;
}

pub trait SpecificParser<V> where V: SpecificVariant {

    fn identifier() -> SpecificParserImpl<specific::variants::Identifier> {
        Default::default()
    }

    fn selector() -> SpecificParserImpl<specific::variants::Selector> {
        Default::default()
    }

    fn ctx() -> SpecificParserImpl<specific::variants::Ctx> {
        Default::default()
    }

    fn parse<I>(input: I) -> Res<I,SpecificExt<V>> where I: Span {

        let contributor = <<V::Contributor::Parser as PrimitiveArchetype>::Parser as PrimitiveParser>::parse;
        let package= <<V::Package::Parser as PrimitiveArchetype>::Parser as PrimitiveParser>::parse;
        let version = <<V::Version::Parser as PrimitiveArchetype>::Parser as PrimitiveParser>::parse;

        tuple((contributor, tag(":"), package, tag(":"), version))(input).map( |(next,((contributor,_,package,_,version)))| {
            (next,SpecificExt::new(contributor,package,version))
        })
    }
}







pub trait ExtParser<V> where V: ExtVariant+?Sized {

    fn new() -> ExtParserImpl<V> {
        Default::default()
    }

    fn block<'x>() -> &'static NestedBlockKind {
        <<V::Type as PrimitiveArchetype>::Parser as TypeParser<V::Type>>::block()
    }


    fn specific_prelude<I>(input: I) -> Res<I,V::Specific> where I: Span {
        preceded(tag("@"),Self::specific)(input)
    }

    fn specific<I>(input:I) -> Res<I,V::Specific>  where I: Span {
        <<V::Specific as PrimitiveArchetype>::Parser as PrimitiveParser>::parse(input)
    }

    fn segment<I>(input:I) ->Res<I,V::Type> where I: Span {
       <<V::Type as TypeVariant>::Parser as TypeParser<V::Type>>::segment(input)
    }


    fn scope<I>(input:I) -> Res<I,V::Scope> where I: Span {
        <<V::Scope as PrimitiveArchetype>::Parser as PrimitiveParser>::parse(input)
    }

    fn outer<I>(input:I) -> Res<I,Ext<V>> where I: Span {
        V::Type::Parser::enter(Self::parse)(input)
    }

    fn parse<I>(input: I) -> Res<I, Ext<V>>
    where
        I: Span
    {
        tuple((Self::scope, V::Type::Parser::generic, Self::specific_prelude))(input).map(|(next,(scope,generic,specific))|
            (next,Ext::new(scope,generic,specific))
        )
    }
}

pub struct ExtParserImpl<V>(PhantomData<V>) where V: ?Sized;
pub struct TypeParserImpl<V>(PhantomData<V>) where V: ?Sized;
pub struct SpecificParserImpl<V>(PhantomData<V>) where V: ?Sized;
impl <V> ExtParser<V> for ExtParserImpl<V> where V: ExtVariant { }
impl <T> TypeParser<T> for TypeParserImpl<T> where T: TypeVariant { }
impl <V> SpecificParser<V> for SpecificParserImpl<V> where V: SpecificVariant { }

impl <X> Default for ExtParserImpl<X> {
    fn default() -> Self {
        Self(PhantomData::default())
    }
}
impl <X> Default for TypeParserImpl<X> {
    fn default() -> Self {
        Self(PhantomData::default())
    }
}

impl <X> Default for SpecificParserImpl<X> {
    fn default() -> Self {
        Self(PhantomData::default())
    }
}


pub trait ParserWrapper<I> where I: Span {
    fn parse<F,Out,Wrap>(&self, f: F) -> impl FnMut(I) -> Res<I,Out> where F: FnMut(I) -> Res<I,Wrap>+Copy;
}


/// for parsing the unique structures of the [Type]'s:  [Class] & [Schema].
/// a [Type] cannot be parsed on its own so [TypeParser] supplies the necessary
/// sup parsers to [ExtParser]
pub trait TypeParser<T> where T: TypeVariant{

    fn identifier<V>() -> TypeParserImpl<Identifier<V>> where V: TypeVariant
    {
        Default::default()
    }

    fn of_type<I>() -> &'static TypeDiscriminant {
        T::of_type()
    }

    fn discriminant<I>(input:I) -> Res<I, T::Discriminant>
    where
        I: Span {

        let parse = <T::Segment::Parser as PrimitiveParser>::parse;
        parse_from_str(parse)(input)
    }

    fn block() -> &'static NestedBlockKind {
        T::block()
    }

    fn segment_prelude<I>(input: I) -> Res<I, T::Segment> where I:Span {
        preceded(peek(Self::open),Self::segment)(input)
    }

    fn segment<I>(input: I) -> Res<I, T::Segment> where I:Span {
        <T::Segment::Parser as PrimitiveParser>::parse(input)
    }

    /// opening character for a NestedBlock
    fn open<I>(input:I) -> Res<I,()> where I: Span {
        tag(Self::block().open())(input)
    }

    /// enter the block and start parsing like mad
    fn enter<I, F, O>(f: F) -> impl FnMut(I) -> Res<I, O>
    where
        F: FnMut(I) -> Res<I, O>, I: Span
    {
        Self::block().unwrap(f)
    }

    fn stack<I>(input: I) -> Res<I, TypeVariantStack<T>> where I: Span{
        let segment = Self::segment;
        let generic = Self::enter(Self::segment);

        pair(segment,many0(generic))(input).map(|(next,(segment,mut generics))|  {
            generics.insert(0, segment);
            let stack = generics.into();
            (next,stack)
        })
    }

    fn generic<I>(input: I) -> Res<I,T> where I: Span{
        into(Self::stack)(input)
    }

}


pub mod case {}

/// scan `opt(f) -> Option<D>`  then [Option::unwrap_or_default]  to generate a [D::default] value
///
pub fn opt_def<I, F, D>(f: F) -> impl Fn(I) -> Res<I, D>
where
    I: Span,
    F: FnMut(I) -> Res<I, D> + Copy,
    D: Default,
{
    move |input| opt(f)(input).map(|(next, opt)| (next, opt.unwrap_or_default()))
}

fn kind<K: TypeVariant, I: Span>(input: I) -> Res<I, K>
where
    K: TypeVariant + From<CamelCase>,
{
    from_camel(input)
}

pub(super) mod util {
    use std::ops::Deref;
    use crate::err::ParseErrs;
    use crate::types::variant::TypeVariant;

    pub struct TypeVariantStack<G> where G: TypeVariant{
        segments: Vec<G::Segment>
    }

    impl <G> TypeVariantStack<G> where G: TypeVariant{
        pub fn two(&self) -> Result<(&G::Segment,Option<&G::Segment>),ParseErrs>  {
            match self.segments.len() {
                1 =>  Ok((self.first().unwrap(),None)),
                2 =>  Ok((self.first().unwrap(),self.get(1))),
                len => {
                    Err(ParseErrs::expected(format!("{}",G::of_type()), "segment count between: 1..2", len.to_string() ))
                }
            }
        }
    }


    impl <G> From<Vec<G::Segment>> for TypeVariantStack<G> where G:TypeVariant{
        fn from(segments: Vec<G::Segment>) -> Self {
            Self { segments }
        }
    }

    impl <G> TypeVariantStack<G> where G: TypeVariant{

        pub fn new() -> Self {
            Default::default()
        }

        pub fn push(& mut self, segment:G::Segment ) {
            self.segments.push(segment);
        }


        pub fn as_string(&self, index: &usize) -> String {
            self.segments.get(index).map(ToString::to_string).unwrap_or_default()
        }

    }

    impl <G> Default for TypeVariantStack<G> where G: TypeVariant{
        fn default() -> Self {
            Self {
                segments: Default::default()
            }
        }
    }

    impl <G> Deref for TypeVariantStack<G> where G: TypeVariant {
        type Target = Vec<G::Segment>;

        fn deref(&self) -> &Self::Target {
            &self.segments
        }
    }

    pub enum Alt<V,S> {
        Variant((V,Option<S>)),
        Specific(S),
    }

    impl <V,S> From<S> for Alt<V,S> {
        fn from(value: S) -> Self {
            Self::Specific(value)
        }
    }

    impl <V,S> From<(V,Option<S>)> for Alt<V,S> {
        fn from(value: (V,Option<S>)) -> Self {
            Self::Variant(value)
        }
    }
}
