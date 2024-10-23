use crate::lib::std::vec::Vec;
use crate::space::case::VarCase;
use crate::space::parse::nomplus::{Input, MyParser, Res};
use point::{PntFragment};
use crate::space::parse::util::{tron, Trace};
use nom_supreme::ParserExt;
pub mod point;

pub type TokenTron = Trace<Token>;

pub enum Token {
    Comment,
    Point(PntFragment),
}

pub type Variable = Trace<VarCase>;

pub type PointTokens = Vec<PntFragment>;

pub(crate) fn tk<'a, I, F, O>(f: F) -> impl FnMut(I) -> Res<I, TokenTron>
where
    I: Input,
    F: FnMut(I) -> Res<I, O> + Copy,
    O: Into<Token>,
{
    move |input| {
        tron(f)(input).map(|(next, output)| {
            let o = output.w.into();
            let trace = Trace {
                w: o,
                range: output.range,
            };

            (next, trace)
        })
    }
}

