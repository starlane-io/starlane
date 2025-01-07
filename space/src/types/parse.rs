use std::fmt::Display;
use crate::parse::util::{preceded, Span};
use crate::parse::{camel_case, lex_block, lex_block_alt, CamelCase, NomErr, Res};
use crate::types::class::Class;
use crate::types::private::{Generic};
use crate::types::{Type, Schema, Ext, TypeDiscriminant};
use futures::FutureExt;
use nom::branch::alt;
use nom::combinator::{into, map, opt, peek, value};
use nom::sequence::{delimited, pair, tuple};
use nom::Parser;
use nom_supreme::tag::complete::tag;
use nom_supreme::ParserExt;
use starlane_space::parse::from_camel;
use std::str::FromStr;
use nom::error::{ErrorKind, FromExternalError};
use nom::multi::separated_list0;
use once_cell::sync::Lazy;
use starlane_space::types::ExtVariant;
use starlane_space::types::specific::SpecificVariant;
use crate::parse::model::{BlockKind, NestedBlockKind};
use crate::types::parse::util::Alt;
use crate::types::specific::SpecificExt;

pub static NESTED_BLOCKS_DEFAULT: Lazy<Option<NestedBlockKind>> =
    Lazy::new(|| None);

/// every 'type' needs to support [PrimitiveArchetype] traits
pub trait PrimitiveArchetype: Display {
    type Parser;
}

pub trait Archetype: Display {
   type Segment: PrimitiveArchetype<Parser:PrimitiveParser>;
   type Parsers: TypeParsers;
}


pub trait PrimitiveParser: Sized {
    type Output;

    fn peek<I>(input: I) -> Res<I,Self::Output> where I: Span {
        peek(Self::parse)(input)
    }

    fn parse<I>(input: I) -> Res<I,Self::Output> where I: Span;

}


pub trait SpecificParser {
    type Output: SpecificVariant;
    fn parse<I>(input: I) -> Res<I,Self::Output> where I: Span {

        let contributor = Self::Output::Contributor::Parser::parse;
        let package = Self::Output::Package::Parser::parse;
        let version = Self::Output::Version::Parser::parse;

        tuple((contributor, tag(":"), package, tag(":"), version))(input).map( |(next,((contributor,_,package,_,version)))| {
            (next,SpecificExt::new(contributor,package,version))
        })
    }
}



mod util {
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



pub trait ExtParser<V> where V: ExtVariant {

    fn block<'x>() -> &'static NestedBlockKind {
        <<V::Type as PrimitiveArchetype>::Parser as TypeParsers>::unwrap()
    }

    fn parse_outer<I>(input: I) -> Res<I,Ext<V>> where I: Span {
        Self::block().unwrap(input)
    }

    fn specific_prelude<I>() -> impl FnMut(I) -> Res<I,V::Specific>+Clone where I: Span {
        preceded(tag("@"),Self::specific)
    }

    fn specific<I>() -> impl FnMut(I) -> Res<I,V::Specific>+Clone where I: Span {
        <<V::Specific as PrimitiveArchetype>::Parser as PrimitiveParser>::parse
    }

    fn variant_prelude<I>() -> impl FnMut(I) -> Res<I,V::Specific>+Clone where I: Span {
        preceded(peek(tag("@")),Self::specific)
    }
    fn segment<I>() -> impl FnMut(I) -> Res<I,V::Type>+Clone where I: Span {
       <<V::Type as Archetype>::Parser as TypeParsers>::segment
    }

    /// return [Alt] enumeration of either [Alt::Variant] or [Atl::Specific]
    fn alt<I>() -> impl FnMut(I) -> Res<I,Alt<V::Type::Segment,V::Specific>>+Clone where I: Span  {
        let segment = Self::segment;
        let specific = Self::specific;
        alt(())
    }

    fn outer<I>(input:I) -> Res<I,Ext<V>> where I: Span {
        V::Type::Parsers::enter(Self::parse)(input)
    }

    fn parse<I>(input: I) -> Res<I, Ext<V>>
    where
        I: Span
    {
        
    }
}

pub type ScopeParser<O> = impl PrimitiveParser<Output=O>;





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
/// a [Type] cannot be parsed on its own so [TypeParsers] supplies the necessary
/// sup parsers to [ExtParser]
pub trait TypeParsers {
    type Type: Generic;
    type Output: TryFrom<Self::Discriminant,Error=strum::ParseError> + FromStr;

    type Discriminant: TryFrom<Self::Segment>;

    type Segment: PrimitiveArchetype<Parser:PrimitiveParser>;

    fn of_type<I>() -> &'static TypeDiscriminant {
        Self::Type::of_type()
    }
    fn discriminant<I>(input:I) -> Res<I, Self::Discriminant>
    where
        I: Span;

    fn block() -> &'static NestedBlockKind {
        Self::Type::block()
    }

    fn segment_prelude<I>(input: I) -> Res<I, Self::Segment> where I:Span {
        preceded(peek(Self::open),Self::segment)(input)
    }

    fn segment<I>(input: I) -> Res<I, Self::Segment> where I:Span {
        Self::Segment::Parser::parse(input)
    }

    /// opening character for a NestedBlock
    fn open<I>() -> impl FnMut(I) -> Res<I,()> where I: Span {
        tag(Self::block().open())
    }

    /// enter the block and start parsing like mad
    fn enter<I, F, O>(mut f: F) -> impl FnMut(I) -> Res<I, O>
    where
        F: FnMut(I) -> Res<I, O>, I: Span
    {
        Self::block().unwrap(f)
    }

   fn variant(_: Self::Discriminant, _: Self::Segment) -> Result<Self::Type,strum::ParseError>  {
        Err(strum::ParseError::VariantNotFound)
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

