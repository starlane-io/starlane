use alloc::string::{String, ToString};
use chrono::{DateTime, Utc};
use core::marker::Sized;
use core::option::Option;
use core::option::Option::{None, Some};
use core::result::Result;
use core::result::Result::{Err, Ok};

use crate::error::UniErr;
use crate::http::HttpMethod;
use crate::id::Uuid;
use crate::parse::Env;
use crate::{cosmic_timestamp, cosmic_uuid};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub enum HttpMethodPattern {
    Any,
    None,
    Pattern(HttpMethod),
}

impl HttpMethodPattern {
    pub fn is_match(&self, x: &HttpMethod) -> Result<(), ()> {
        match self {
            Self::Any => Ok(()),
            Self::Pattern(exact) => exact.is_match(x),
            Self::None => Err(()),
        }
    }

    pub fn is_match_opt(&self, x: Option<&HttpMethod>) -> Result<(), ()> {
        match self {
            Self::Any => Ok(()),
            Self::Pattern(exact) => match x {
                None => Err(()),
                Some(x) => self.is_match(x),
            },
            Self::None => Err(()),
        }
    }
}

impl ToString for HttpMethodPattern {
    fn to_string(&self) -> String {
        match self {
            Self::Any => "*".to_string(),
            Self::None => "!".to_string(),
            Self::Pattern(pattern) => pattern.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub enum ValuePattern<T> {
    Any,
    None,
    Pattern(T),
}

impl<T> ValuePattern<T>
where
    T: ToString,
{
    pub fn stringify(self) -> ValuePattern<String> {
        match self {
            ValuePattern::Any => ValuePattern::Any,
            ValuePattern::None => ValuePattern::None,
            ValuePattern::Pattern(t) => ValuePattern::Pattern(t.to_string()),
        }
    }
}

impl<T> ToString for ValuePattern<T>
where
    T: ToString,
{
    fn to_string(&self) -> String {
        match self {
            ValuePattern::Any => "*".to_string(),
            ValuePattern::None => "!".to_string(),
            ValuePattern::Pattern(pattern) => pattern.to_string(),
        }
    }
}

impl<T> ValuePattern<T> {
    pub fn modify<X, F>(self, mut f: F) -> Result<ValuePattern<X>, UniErr>
    where
        F: FnMut(T) -> Result<X, UniErr>,
    {
        Ok(match self {
            ValuePattern::Any => ValuePattern::Any,
            ValuePattern::None => ValuePattern::None,
            ValuePattern::Pattern(from) => ValuePattern::Pattern(f(from)?),
        })
    }

    pub fn wrap<X>(self, x: X) -> ValuePattern<X> {
        match self {
            ValuePattern::Any => ValuePattern::Any,
            ValuePattern::None => ValuePattern::None,
            ValuePattern::Pattern(_) => ValuePattern::Pattern(x),
        }
    }

    pub fn is_match<X>(&self, x: &X) -> Result<(), ()>
    where
        T: ValueMatcher<X>,
    {
        match self {
            ValuePattern::Any => Ok(()),
            ValuePattern::Pattern(exact) => exact.is_match(x),
            ValuePattern::None => Err(()),
        }
    }

    pub fn is_match_opt<X>(&self, x: Option<&X>) -> Result<(), ()>
    where
        T: ValueMatcher<X>,
    {
        match self {
            ValuePattern::Any => Ok(()),
            ValuePattern::Pattern(exact) => match x {
                None => Err(()),
                Some(x) => self.is_match(x),
            },
            ValuePattern::None => Err(()),
        }
    }
}

pub trait ValueMatcher<X> {
    fn is_match(&self, x: &X) -> Result<(), ()>;
}

pub struct RegexMatcher {
    pub pattern: String,
}

impl ToString for RegexMatcher {
    fn to_string(&self) -> String {
        self.pattern.clone()
    }
}

impl RegexMatcher {
    pub fn new(string: String) -> Self {
        Self { pattern: string }
    }
}

impl ValueMatcher<String> for RegexMatcher {
    fn is_match(&self, x: &String) -> Result<(), ()> {
        let matches = x.matches(x);
        if matches.count() > 0 {
            Ok(())
        } else {
            Err(())
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct StringMatcher {
    pub pattern: String,
}

impl ToString for StringMatcher {
    fn to_string(&self) -> String {
        self.pattern.clone()
    }
}

impl StringMatcher {
    pub fn new(string: String) -> Self {
        Self { pattern: string }
    }
}

impl ValueMatcher<String> for StringMatcher {
    fn is_match(&self, x: &String) -> Result<(), ()> {
        if self.pattern == *x {
            Ok(())
        } else {
            Err(())
        }
    }
}

pub trait Convert<A> {
    fn convert(self) -> Result<A, UniErr>;
}

pub trait ConvertFrom<A>
where
    Self: Sized,
{
    fn convert_from(a: A) -> Result<Self, UniErr>;
}

pub fn uuid() -> Uuid {
    unsafe { cosmic_uuid() }
}

pub fn timestamp() -> DateTime<Utc> {
    unsafe { cosmic_timestamp() }
}

pub trait ToResolved<R>
where
    Self: Sized,
{
    fn collapse(self) -> Result<R, UniErr> {
        self.to_resolved(&Env::no_point())
    }

    fn to_resolved(self, env: &Env) -> Result<R, UniErr>;
}

pub fn log<R>(result: Result<R, UniErr>) -> Result<R, UniErr> {
    match result {
        Ok(r) => Ok(r),
        Err(err) => {
            err.print();
            Err(err)
        }
    }
}
