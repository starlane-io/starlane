use std::fmt::Display;
use std::marker::PhantomData;
use crate::parse::util::{preceded, Span};
use crate::parse::{camel_case, lex_block, lex_block_alt, CamelCase, NomErr, Res};
use crate::types::class::Class;
use crate::types::private::{Generic};
use crate::types::{Type, Schema, Ext, TypeDiscriminant, specific};
use futures::FutureExt;
use nom::branch::alt;
use nom::combinator::{into, map, opt, peek, value};
use nom::sequence::{delimited, pair, terminated, tuple};
use nom::Parser;
use nom_supreme::tag::complete::tag;
use nom_supreme::ParserExt;
use starlane_space::parse::{from_camel, parse_from_str};
use std::str::FromStr;
use nom::error::{ErrorKind, FromExternalError};
use nom::multi::{many0, separated_list0};
use once_cell::sync::Lazy;
use starlane_space::types::ExtVariant;
use starlane_space::types::private::{variants, TypeVariant};
use starlane_space::types::specific::SpecificVariant;
use crate::err::ParseErrs;
use crate::parse::model::{BlockKind, NestedBlockKind};
use crate::types::parse::util::{Alt, VariantStack};
use crate::types::specific::SpecificExt;

pub static NESTED_BLOCKS_DEFAULT: Lazy<Option<NestedBlockKind>> =
    Lazy::new(|| None);

/// every 'type' needs to support [PrimitiveArchetype] traits
pub trait PrimitiveArchetype: Display {
    type Parser: ?Sized;
}

pub trait Archetype<T>: Display where T: Generic {
   type Segment: PrimitiveArchetype<Parser:PrimitiveParser>;
   type Parser: TypeParser<T>;
}


pub trait PrimitiveParser {
    type Output;

    fn peek<I>(input: I) -> Res<I,Self::Output> where I: Span {
        peek(Self::parse)(input)
    }

    fn parse<I>(input: I) -> Res<I,Self::Output> where I: Span;

}



pub trait SpecificParser<V> where V: SpecificVariant {

    fn identifier() -> ParserImpl<specific::variants::Identifier> {
        Default::default()
    }

    fn selector() -> ParserImpl<specific::variants::Selector> {
        Default::default()
    }

    fn ctx() -> ParserImpl<specific::variants::Ctx> {
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



pub mod util {
    use std::ops::Deref;
    use crate::err::ParseErrs;
    use crate::types::private::Generic;

    pub struct VariantStack<G> where G: Generic{
        segments: Vec<G::Segment>
    }

    impl <G> VariantStack<G> where G: Generic{
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


    impl <G> From<Vec<G::Segment>> for VariantStack<G> where G:Generic{
        fn from(segments: Vec<G::Segment>) -> Self {
            Self { segments }
        }
    }

    impl <G> VariantStack<G> where G: Generic{

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

    impl <G> Default for VariantStack<G> where G: Generic{
        fn default() -> Self {
            Self {
                segments: Default::default()
            }
        }
    }

    impl <G> Deref for VariantStack<G> where G: Generic {
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



pub trait ExtParser<V> where V: ExtVariant+?Sized {

    fn new() -> ParserImpl<V> {
        ParserImpl::default()
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
       <<V::Type as Archetype>::Parser as TypeParser<V::Type>>::segment(input)
    }


    /*

    /// return [Alt] enumeration of either [Alt::Variant] or [Atl::Specific]
    fn alt<I>(input: I) -> Res<I,Alt<V::Type::Segment,V::Specific>> where I: Span  {
        let specific = Self::specific_prelude.map(Alt::from);
        let variant = tuple((V::Type::Parser::segment_prelude, opt(Self::specific_prelude))).map(Alt::from);
        alt((specific,variant))(input)
    }

     */

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
        tuple((Self::scope,V::Type::Parser::variant,Self::specific_prelude))(input).map(|(next,(scope,generic,specific))|
            (next,Ext::new(scope,generic,specific))
        )
    }
}

pub struct ParserImpl<V>(PhantomData<V>) where V: ?Sized;
impl <V> ExtParser<V> for ParserImpl<V> where V: ExtVariant{}
impl <T> TypeParser<T> for ParserImpl<T> where T: TypeVariant { }
impl <V> SpecificParser<V> for ParserImpl<V> where V: SpecificVariant{}

impl <X> Default for ParserImpl<X> {
    fn default() -> Self {
        ParserImpl(PhantomData::default())
    }
}


pub trait ParserWrapper<I> where I: Span {
    fn parse<F,Out,Wrap>(&self, f: F) -> impl FnMut(I) -> Res<I,Out> where F: FnMut(I) -> Res<I,Wrap>+Copy;
}


/*
pub trait SeparatedSegmentParser  {
   type Segment: Primitive<Parser:PrimitiveParser>;
   fn delimiter() -> &'static str;

   fn parse<I>(&self, input:I) -> Res<I,Vec<Self::Segment>> where I: Span {
       separated_list0(tag(Self::delimiter()),Self::Segment::Parser::parse)(input)
   }
}

pub trait VariantSegmentParser {

    type Segment: Primitive<Parser: SeparatedSegmentParser>;

    type Specific: Primitive;

    fn block() -> NestedBlockKind;


    fn unwrap<I: Span, F, O>(f: F) -> impl FnMut(I) -> Res<I, O>
    where
        F: FnMut(I) -> Res<I, O>,
    {
        Self::block().unwrap(f)
    }

    fn unwrap_and_parse<I: Span>(&self, input: I) -> Res<I,Vec<Self::Segment>>
    {
        Self::unwrap(|input|self.parse(input))
    }

    fn parse<I>(&self,input: I) -> Res<I, (Vec<Self::Segment>,Option<Self::Specific>)>
    where
        I: Span
    {

        enum Alt<S,V> {
            Specific(S),
            Variant((V,Option<S>))
        }

        impl <S,V> From<S> for Alt<S,V> {
            fn from(value: S) -> Self {
                Self::Specific(value)
            }
        }

        impl <S,V> From<(V,Option<S>)> for Alt<S,V> {
            fn from(value: (V,Option<S>)) -> Self {
                Self::Variant(value)
            }
        }

        let segment = Self::Segment::Parser::parse;
        let open = tag(Self::block().open());
        let specific = preceded(tag("@"),Self::Specific::Parser::parse);

        let unwrap = |f| Self::block().unwrap(f);

        let alt = alt((into(unwrap(pair(segment,opt(specific.clone())))),into(specific)));

           pair(segment,alt)(input).map(|(next,(segment,alt))| {
             match alt {
                 Alt::Variant((variant,specific)) => {
                     let segments = vec![segment,variant];
                     (next,(segments,specific))
                 }
                 Alt::Specific(specific) => {
                     let segments = vec![segment];
                     (next,(segments,specific))
                 }
             }
        })

    }
}

 */



/// for parsing the unique structures of the [Type]'s:  [Class] & [Schema].
/// a [Type] cannot be parsed on its own so [TypeParser] supplies the necessary
/// sup parsers to [ExtParser]
pub trait TypeParser<T> where T: TypeVariant{

    fn identifier<G>() -> ParserImpl<variants::Identifier<G>> where G: Generic {
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

    fn stack<I>(input: I) -> Res<I,VariantStack<T>> where I: Span{
        let segment = Self::segment;
        let variant = Self::enter(Self::segment);

        pair(segment,many0(variant))(input).map(|(next,(segment,mut variants))|  {
            variants.insert(0,segment);
            let stack = variants.into();
            (next,stack)
        })
    }

    fn variant<I>(input: I) -> Res<I,T> where I: Span{
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

fn kind<K: Generic, I: Span>(input: I) -> Res<I, K>
where
    K: Generic + From<CamelCase>,
{
    from_camel(input)
}

