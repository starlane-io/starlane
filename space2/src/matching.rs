use alloc::vec::Vec;
use core::fmt::Display;

pub trait Matcher: Display{
    type Type;
    /// Returns the result of [Matcher::is] converted into a [Result]
    /// * [Result::Ok] if true
    /// * [Result::Err] if false
    /// the return Result is not really an error, just a convenient way
    /// to check if a long list of items are matches using the `?` operator to conveniently
    /// abort the operation without the need for a bunch of if/then logic.
    fn result(&self, item: &Self::Type  ) -> Result<(),()> {
        match self.is(item) {
            true => Ok(()),
            false => Err(())
        }
    }

    /// Returns `true` if match is determined.
    ///
    /// `IMPORTANT`: `match` does not me `equals` in this trait (although it can)
    fn is(&self, item: &Self::Type) -> bool;
}


impl <E> Matcher for E where E: PartialEq<E>+Display{
    type Type = E;

    fn is(&self, item: &Self::Type) -> bool {
        self.eq(item)
    }
}






#[derive(Debug, Clone, strum_macros::Display,strum_macros::IntoStaticStr)]
pub enum Pattern<P> where P: Matcher {
    /// [Pattern::Always] will match anything
    #[strum(to_string = "*")]
    Always,
    /// [Pattern::Match] will match any that evaluate as `Equal`
    #[strum(to_string = "{0}")]
    Match(P),
    /// [Pattern::Not] will match any that is `Not Equal`
    #[strum(to_string = "!")]
    Not(P),
    /// [Pattern::Or] will match if  `Any` in the vec are a match
    #[strum(to_string = "")]
    Or(Vec<P>),
     /// [Pattern::And] will match if `All` in the vec are a match
     #[strum(to_string = "!")]
    And(Vec<P>)
}

impl <P,I> Matcher for Pattern<P> where P: Matcher<Type=I> {
    type Type = I;

    fn is(&self, item: &Self::Type) -> bool {
        match self {
            Pattern::Always => true,
            Pattern::Match(matcher) => matcher.is(item),
            Pattern::Not(matcher) => !matcher.is(item),
            Pattern::Or(matchers) =>  {
                matchers.iter().any(|m| m.is(item))
            }
            Pattern::And(matchers) => {
                matchers.iter().all(|m| m.is(item))
            }
        }
    }
}