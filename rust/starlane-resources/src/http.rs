use std::collections::HashMap;
use crate::data::BinSrc;
use std::str::FromStr;
use crate::error::Error;
use serde::{Serialize,Deserialize};

pub type Headers = HashMap<String,String>;

#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct HttpRequest{
   pub path: String,
   pub method: HttpMethod,
   pub headers: Headers,
   pub body: BinSrc
}

#[derive(Debug,Clone,Serialize,Deserialize)]
pub enum HttpMethod {
    Get,
    Put,
    Post,
    Delete
}


impl FromStr for HttpMethod {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().trim() {
            "GET" => Ok(Self::Get),
            "PUT" => Ok(Self::Put),
            "POST" => Ok(Self::Post),
            "DELETE" => Ok(Self::Delete),
            &_ => Err(format!("method not recognized: {}", s).into())
        }
    }
}

impl ToString for HttpMethod {
    fn to_string(&self) -> String {
        match self {
            HttpMethod::Get => "Get".to_string(),
            HttpMethod::Put => "Put".to_string(),
            HttpMethod::Post => "Post".to_string(),
            HttpMethod::Delete => "Delete".to_string()
        }
    }
}