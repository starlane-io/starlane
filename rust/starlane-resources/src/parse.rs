use nom::error::{context, ErrorKind};
use nom::{InputTakeAtPosition, AsChar};
use crate::{Res, ResourceKindParts, Specific, DomainCase, SkewerCase, Version, ResourceType, ResourceKind, ResourcePathSegmentKind};
use nom::character::complete::{alpha0, alpha1, anychar, digit0, digit1, one_of};
use nom::combinator::{not, opt};
use nom::multi::{many1, many_m_n, separated_list0, separated_list1};
use nom::sequence::{delimited, preceded, terminated, tuple};
use nom::bytes::complete::{tag, take};
use crate::error::Error;
use std::convert::TryInto;
use serde::{Deserialize,Serialize};


fn any_resource_path_segment<T>(i: T) -> Res<T, T>
    where
        T: InputTakeAtPosition,
        <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            !(char_item == '-')
            && !(char_item == '.')
            && !(char_item == '/')
            && !(char_item == '_')
            && !(char_item.is_alpha() || char_item.is_dec_digit())
        },
        ErrorKind::AlphaNumeric,
    )
}

fn loweralphanumerichyphen1<T>(i: T) -> Res<T, T>
    where
        T: InputTakeAtPosition,
        <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            !(char_item == '-')
                && !((char_item.is_alpha() && char_item.is_lowercase()) || char_item.is_dec_digit())
        },
        ErrorKind::AlphaNumeric,
    )
}

fn parse_domain(input: &str) -> Res<&str, DomainCase> {
    context(
        "domain",
        tuple((
            many1(terminated(loweralphanumerichyphen1, tag("."))),
            loweralphanumerichyphen1,
        )),
    )(input)
        .map(|(next_input, mut res)| {
            if !res.1.is_empty() {
                res.0.push(res.1);
            }
            (next_input, DomainCase::new(res.0.join(".").as_str()))
        })
}

fn parse_version_major_minor_patch(input: &str) -> Res<&str, (usize, usize, usize)> {
    context(
        "version_major_minor_patch",
        tuple((
            terminated(digit1, tag(".")),
            terminated(digit1, tag(".")),
            terminated(digit1, not(digit1)),
        )),
    )(input)
        .map(|(next_input, res)| {
            (
                next_input,
                (
                    res.0.parse().unwrap(),
                    res.1.parse().unwrap(),
                    res.2.parse().unwrap(),
                ),
            )
        })
}

fn parse_version(input: &str) -> Res<&str, Version> {
    context(
        "version",
        tuple((parse_version_major_minor_patch, opt(preceded(tag("-"), parse_skewer)))),
    )(input)
        .map(|(next_input, ((major, minor, patch), release))| {
            (next_input, Version::new(major, minor, patch, release))
        })
}

fn parse_skewer(input: &str) -> Res<&str, SkewerCase> {
    context("skewer-case", loweralphanumerichyphen1)(input)
        .map(|(input, skewer)| (input, SkewerCase::new(skewer)))
}

fn parse_specific(input: &str) -> Res<&str, Specific> {
    context(
        "specific",
        tuple((
            terminated(parse_domain, tag(":")),
            terminated(loweralphanumerichyphen1, tag(":")),
            terminated(loweralphanumerichyphen1, tag(":")),
            parse_version,
        )),
    )(input)
        .map(|(next_input, (vendor, product, variant, version))| {
            (
                next_input,
                Specific {
                    vendor: vendor,
                    product: product.to_string(),
                    variant: variant.to_string(),
                    version: version,
                },
            )
        })
}


pub fn parse_resource_kind(input: &str) -> Res<&str, Result<ResourceKind,Error>> {
    context(
        "kind",
        delimited(
            tag("<"),
            tuple((
                alpha1,
                opt(delimited(
                    tag("<"),
                    tuple((alpha1, opt(delimited(tag("<"), parse_specific, tag(">"))))),
                    tag(">"),
                )),
            )),
            tag(">"),
        ),
    )(input)
        .map(|(input, (rt, more))| {
            let kind = match &more {
                None => Option::None,
                Some((kind, _)) => Option::Some((*kind).clone().to_string()),
            };
            let spec = match &more {
                None => Option::None,
                Some((_, Option::Some(spec))) => Option::Some(spec.clone()),
                _ => Option::None,
            };
            (
                input,
                ResourceKindParts {
                    resource_type: rt.to_string(),
                    kind: kind,
                    specific: spec,
                }.try_into(),
            )
        })
}
pub fn parse_resource_path(input: &str) -> Res<&str, ResourcePath> {
    context(
        "resource-path",
        separated_list0(
            nom::character::complete::char(':'),
            any_resource_path_segment,
        ),
    )(input).map( |(next_input, segments) | {
        let segments : Vec<String> = segments.iter().map(|s|s.to_string()).collect();
        (next_input,ResourcePath {
            segments
        })
    } )
}

pub fn parse_resource_path_and_kind(input: &str) -> Res<&str, Result<ResourcePathAndKind,Error>> {
    context(
        "parse_resource_path_and_kind",
        tuple(
            (parse_resource_path,
             parse_resource_kind)
        ),
    )(input).map( |(next_input, (path, resource_kind)) | {

        (next_input,

         {
             match resource_kind {
                 Ok(kind) => {
                     ResourcePathAndKind::new(path,kind)
                 }
                 Err(error) => {
                     Err(error.into())
                 }
             }
         }


        )
    } )
}


/*
pub fn parse_resource_path_with_type_kind_specific_and_property(input: &str) -> Res<&str, (Vec<&str>,Option<ResourceTypeKindSpecificParts>)> {
    context(
        "parse_resource_path_with_type_kind_specific_and_property",
        tuple(
            (terminated(parse_resource_path_with_type_kind_specific, tag("::") ),
             parse_resource_type_kind_specific))
    )(input)
}
 */
#[derive(Debug,Clone,Serialize,Deserialize,Eq,PartialEq,Hash)]
pub enum ResourceExpression {
    Path(ResourcePath),
    Kind(ResourcePathAndKind)
}

#[derive(Debug,Clone,Serialize,Deserialize,Eq,PartialEq,Hash)]
pub struct ResourcePath {
  pub segments: Vec<String>
}

impl ResourcePath {
    pub fn new( segments: Vec<String> ) -> Self {
        Self{
            segments
        }
    }
}

#[derive(Debug,Clone,Serialize,Deserialize,Eq,PartialEq,Hash)]
pub struct ResourcePathAndKind {
    pub path: ResourcePath,
    pub kind: ResourceKind,
}

impl ResourcePathAndKind {
    pub fn new(path: ResourcePath, kind: ResourceKind) -> Result<Self,Error> {
        let path_segment_kind: ResourcePathSegmentKind = kind.resource_type().path_segment_kind();
        // if the path segment is illegal then there will be a Result::Err returned
        path_segment_kind.from_str(path.segments.last().ok_or("expected at least one resource path segment" )?.as_str() )?;

        Ok(ResourcePathAndKind{
            path,
            kind
        })
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;
    use std::str::FromStr;

    use crate::error::Error;
    use crate::parse::{parse_resource_path, parse_resource_path_and_kind};

    #[test]
    fn test_parse_resource_path() -> Result<(), Error> {
        let (leftover, path)= parse_resource_path("hello:my:future")?;
        assert!(leftover.len()==0);
        assert!(path.segments.len()==3);
        let (leftover, path)= parse_resource_path("hello:my:future<")?;
        assert!(leftover.len()==1);

        let (leftover, path)= parse_resource_path("that.bi-zi:ba_tiko:/NOW_HE_DEAD")?;

        assert!(leftover.len()==0);
        assert!(path.segments.len()==3);

        Ok(())
    }

    #[test]
    fn test_parse_resource_path_and_kind() -> Result<(), Error> {
        let (leftover, result)= parse_resource_path_and_kind("hello:my<SubSpace>")?;
        let path = result?;
        assert!(leftover.len()==0);
        assert!(path.path.segments.len()==2);

        let (leftover, result)= parse_resource_path_and_kind("hello:my:bundle:1.2.0<ArtifactBundle>")?;
        let path = result?;
        assert!(leftover.len()==0);
        assert!(path.path.segments.len()==4);


        Ok(())
    }
}