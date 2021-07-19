use std::str::FromStr;

use nom::bytes::complete::{tag, take_until, take_while, take_while1};
use nom::character::{is_alphabetic, is_alphanumeric};
use nom::character::complete::{alpha1, alphanumeric0, alphanumeric1};
use nom::error::{context, VerboseError};
use nom::IResult;
use nom::multi::many1;
use nom::sequence::delimited;

use starlane_resources::parse_kind;

use crate::error::Error;
use crate::resource::{FieldSelection, ResourceSelector, ResourceType};

type Res<T, U> = IResult<T, U, VerboseError<T>>;

pub struct MultiResourceSelector{
  pub rt: ResourceType
}

impl Into<ResourceSelector> for MultiResourceSelector{
    fn into(self) -> ResourceSelector {
        let mut selector = ResourceSelector::new();
        selector.add_field(FieldSelection::Type(self.rt));
        selector
    }
}

/*
fn resource_type( input: &str ) -> Res<&str,Result<ResourceType,Error>> {
    context( "resource_type",
       delimited( tag("<"), alpha1, tag(">")  )
    )(input).map( |(next_input, mut res )|{
        (next_input,ResourceType::from_str(res))
    })
}

 */


impl FromStr for MultiResourceSelector {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (leftover,parts) = parse_kind( s )?;

        if !leftover.is_empty() {
            return Err(format!("unexpected leftover '{}' when parsing '{}'", leftover, s).into());
        }
        let resource_type = ResourceType::from_str(parts.resource_type.as_str())?;

        Ok(MultiResourceSelector{
            rt: resource_type
        })
    }
}


#[cfg(test)]
mod test {
    use std::str::FromStr;

    use crate::error::Error;
    use crate::resource::selector::MultiResourceSelector;

    #[test]
    pub fn test() -> Result<(),Error> {
        MultiResourceSelector::from_str("<SubSpace>")?;

        Ok(())
    }
}
