use alloc::string::String;
use core::fmt;
use core::fmt::{Display, Formatter};
use core::ops::Deref;
use core::str::FromStr;
use crate::space::parse::nom::err::ParseErr;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct VarCase {
    string: String,
}
impl Deref for VarCase {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.string
    }
}

impl FromStr for VarCase {
    type Err = ParseErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        result(all_consuming(var_chars)(new_span(s)))?;
        Ok(Self {
            string: s.to_string(),
        })
    }
}

impl Display for VarCase {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.string.as_str())
    }
}


#[cfg(feature="serde")]
pub mod serde {
    use alloc::string::String;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use crate::space::parse::case::VarCase;

    impl Serialize for VarCase {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str(self.string.as_str())
        }
    }

    impl<'de> Deserialize<'de> for VarCase {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            let string = String::deserialize(deserializer)?;

            let result = result(var_case(new_span(string.as_str())));
            match result {
                Ok(var) => Ok(var),
                Err(err) => Err(serde::de::Error::custom(err.to_string())),
            }
        }
    }



}
pub(crate) mod nom {
    use nom::combinator::recognize;
    use nom::sequence::pair;
    use nom::character::complete::{alpha1, alphanumeric1};
    use nom::multi::many0;
    use nom::branch::alt;
    use nom_supreme::ParserExt;
    use nom_supreme::tag::complete::tag;
    use crate::space::parse::nom::Input;

    fn var_chars<I: Input>(input: I) -> Res<I, I> {
        recognize(pair(alpha1, many0(alt((alphanumeric1, tag("_"))))).context(VarErrCtx::VarName.into()))(input)
    }
}

