use starlane_resources::ResourcePath;
use crate::cache::{Cacheable, Data};
use crate::artifact::ArtifactRef;
use crate::resource::config::Parser;
use std::sync::Arc;
use crate::error::Error;
use starlane_resources::http::HttpMethod;
use starlane_resources::ArtifactKind;
use std::collections::HashSet;
use std::iter::FromIterator;
use crate::config::http_router::parse::{parse_http_mappings, parse_reverse_proxy_config};
use regex::Regex;

pub struct HttpRouterConfig {
    pub artifact: ResourcePath,
    pub mappings: Vec<HttpMapping>
}

impl Cacheable for HttpRouterConfig {
    fn artifact(&self) -> ArtifactRef {
        ArtifactRef {
            path: self.artifact.clone(),
            kind: ArtifactKind::HttpRouter
        }
    }

    fn references(&self) -> Vec<ArtifactRef> {
        vec![]
    }
}

pub struct HttpRouterConfigParser;

impl HttpRouterConfigParser {
    pub fn new() -> Self {
        Self {}
    }
}

impl Parser<HttpRouterConfig> for HttpRouterConfigParser {
    fn parse(&self, artifact: ArtifactRef, _data: Data) -> Result<Arc<HttpRouterConfig>, Error> {
        let data = String::from_utf8((*_data).clone() )?;

        let (leftover,mappings) = parse_reverse_proxy_config(data.as_str())?;

        if leftover.len() > 0 {
            return Err(format!("could not parse portion of ReverseProxyConfig: {}",leftover).into());
        }

        let mappings = mappings?;

        Ok(Arc::new(HttpRouterConfig {
            artifact: artifact.path,
            mappings
        }))
    }
}

#[derive(Clone)]
pub struct HttpMapping {
    pub methods: HashSet<HttpMethod>,
    pub path_pattern: Regex,
    pub resource_pattern: String
}

impl HttpMapping {
    pub fn new( methods: Vec<HttpMethod>, path_pattern: String, resource_pattern: String ) -> Result<Self,Error> {
        let path_pattern = Regex::new(path_pattern.as_str() )?;
        let methods = HashSet::from_iter( methods.iter().map(|m|m.clone()) );
        Ok(Self {
            methods,
            path_pattern,
            resource_pattern
        })
    }
}

mod parse {
    use starlane_resources::http::HttpMethod;
    use crate::error::Error;
    use starlane_resources::Res;
    use nom::error::{context, ErrorKind};
    use nom::sequence::{tuple, delimited, terminated, preceded};
    use nom::character::complete::{alpha1, multispace0, multispace1};
    use std::str::FromStr;
    use nom::branch::alt;
    use nom::{InputTakeAtPosition, AsChar};
    use nom::bytes::complete::{tag, take_until};
    use starlane_resources::parse::{parse_resource_path, not_whitespace_or_semi};
    use crate::config::http_router::{HttpMapping, HttpRouterConfig};
    use nom::multi::{separated_list0, many0};


    fn asterisk<T>(i: T) -> Res<T, T>
        where
            T: InputTakeAtPosition,
            <T as InputTakeAtPosition>::Item: AsChar,
    {
        i.split_at_position1_complete(
            |item| {
                let char_item = item.as_char();
                !(char_item == '*')
            },
            ErrorKind::AlphaNumeric,
        )
    }
    pub fn parse_http_methods(input: &str) -> Res<&str, Result<Vec<HttpMethod>,Error>> {
        context(
            "parse_http_methods",
            alt((alpha1, asterisk) ),
        )(input).map(|(input_next,method)| {
            if method == "*" {
                (input_next, Ok(vec![HttpMethod::Get,HttpMethod::Post,HttpMethod::Put,HttpMethod::Delete]))
            } else {
                let method = HttpMethod::from_str(method);
                match method {
                    Ok(method) => {
                        (input_next, Result::Ok(vec![method]))
                    }
                    Err(error) => {
                        (input_next, Err(error.into()))
                    }
                }
            }
        } )
    }

    pub fn parse_path_pattern(input: &str) -> Res<&str, &str> {

            input.split_at_position_complete(char::is_whitespace)
    }

    pub fn parse_http_mapping(input: &str) -> Res<&str, Result<HttpMapping,Error>> {
        context(
            "parse_http_mapping",

            tuple( (delimited( multispace0, parse_http_methods, multispace1 ),
                        terminated( take_until("->"), tag("->")),
                            preceded( multispace0, not_whitespace_or_semi )
                         ) ),

        )(input).map( |(input_next, (methods,path_pattern,resource_pattern))| {
                  match methods {
                      Ok(methods) => {
                          match HttpMapping::new( methods, path_pattern.trim().to_string(), resource_pattern.to_string() ) {
                              Ok(mapping) => {
                                  (input_next,Ok(mapping))
                              }
                              Err(error) => {
                                  (input_next,Err(error.into()))
                              }
                          }
                      }
                      Err(error) => {
                          (input_next,Err(error.into()))
                      }
                  }
            } )
    }

    pub fn parse_http_mappings(input: &str) -> Res<&str, Result<Vec<HttpMapping>,Error>> {
        context( "parse_http_mappings",
                 many0(terminated(parse_http_mapping, tag(";") ) )
        )(input).map( |(input_next,mappings)| {

            for mapping in &mappings {
                if mapping.is_err() {
                    return (input_next, Err(mapping.clone().err().unwrap().into()));
                }
            }

            let mappings = mappings.iter().map( |mapping| {
                mapping.as_ref().unwrap().clone()
            } ).collect();

            (input_next,Ok(mappings))
        } )
    }


    pub fn parse_reverse_proxy_config(input: &str) -> Res<&str, Result<Vec<HttpMapping>,Error>> {
        context( "parse_revesre_proxy_config",
                 delimited( multispace0, parse_http_mappings, multispace0)
        )(input)
    }

}

#[cfg(test)]
mod tests {
    use crate::config::http_router::parse::{parse_http_mapping, parse_http_methods, parse_http_mappings, parse_reverse_proxy_config};
    use crate::error::Error;
    use starlane_resources::http::HttpMethod;

    #[test]
    fn test_http_mapping() -> Result<(), Error> {

        let (leftover, methods) = parse_http_methods("get").unwrap();
        assert!(leftover.len()==0);
        assert!(methods.is_ok());
        let mut methods = methods.unwrap();
        assert!(methods.len()==1);
        assert!(methods.remove(0) == HttpMethod::Get );


        let (leftover, methods) = parse_http_methods("*").unwrap();
        assert!(leftover.len()==0);
        assert!(methods.is_ok());
        let mut methods = methods.unwrap();
        assert!(methods.len()==4);



        let (leftover, mapping)= parse_http_mapping("GET    /hello->space:app:mechtron").unwrap();
        assert!(leftover.len()==0);
        let mapping = mapping.unwrap();
        assert!(mapping.methods.len()==1);
        assert!(mapping.methods.contains(&HttpMethod::Get));
        assert!(mapping.path_pattern.is_match("/hello"));
        assert!(mapping.resource_pattern.to_string().as_str()=="space:app:mechtron");


        let (leftover, mapping)= parse_http_mapping("GET /hello -> space:app:mechtron").unwrap();
        assert!(leftover.len()==0);
        let mapping = mapping.unwrap();
        assert!(mapping.methods.len()==1);
        assert!(mapping.methods.contains(&HttpMethod::Get));
        assert!(mapping.path_pattern.is_match("/hello"));
        assert!(mapping.resource_pattern.to_string().as_str()=="space:app:mechtron");

        Ok(())
    }

    #[test]
    fn test_reverse_proxy_config() -> Result<(), Error> {
        let config = r#"
GET /hello -> space:app:hello;
  POST  /profiles -> space:app:profiles;

    * /resources/* -> space:app:resources;


"#;

        let (leftover, mappings) =  parse_reverse_proxy_config(config).unwrap();

        assert!(leftover.len()==0);
        let mappings = mappings?;
        assert!(mappings.len()==3);

        Ok(())
    }


    }