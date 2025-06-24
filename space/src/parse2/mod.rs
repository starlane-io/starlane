use itertools::Itertools;
use nom::bytes::complete::tag;
use nom::character::complete::alpha1;
use nom::IResult;
use nom::multi::separated_list1;
use nom_locate::LocatedSpan;
use nom_supreme::ParserExt;
use crate::parse::NomErr;
use crate::parse::util::Span;

type Input<'a> = LocatedSpan<&'a str, ()>;
pub type Res<'a,O> = IResult<Input<'a>, O>;
pub fn segments(i: Input) -> Res<Vec<String>> {
   separated_list1(tag(":"),alpha1)(i).map(|(next,mut segments)| {
       let mut rtn = vec![];
       for segment in segments {
           rtn.push(segment.to_string());
       }
        (next,rtn)
    })
}


#[test]
fn test() {
    let data = "you:are: awesome";
}

