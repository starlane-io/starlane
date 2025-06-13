use crate::err::{ParseErrs, PrintErr};
use crate::parse::Env;
use crate::wasm::Timestamp;
use crate::wave::core::http2::HttpMethod;

use crate::loc;
use chrono::Utc;
use core::fmt::Display;
use core::marker::Sized;
use core::option::Option;
use core::option::Option::{None, Some};
use core::result::Result;
use core::result::Result::{Err, Ok};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub enum HttpMethodPattern {
    Always,
    Never,
    Pattern(HttpMethod),
}

impl HttpMethodPattern {
    pub fn is_match(&self, x: &HttpMethod) -> Result<(), ()> {
        match self {
            Self::Always => Ok(()),
            Self::Pattern(exact) => exact.is_match(x),
            Self::Never => Err(()),
        }
    }

    pub fn is_match_opt(&self, x: Option<&HttpMethod>) -> Result<(), ()> {
        match self {
            Self::Always => Ok(()),
            Self::Pattern(exact) => match x {
                None => Err(()),
                Some(x) => self.is_match(x),
            },
            Self::Never => Err(()),
        }
    }
}

impl ToString for HttpMethodPattern {
    fn to_string(&self) -> String {
        match self {
            Self::Always => "*".to_string(),
            Self::Never => "!".to_string(),
            Self::Pattern(pattern) => pattern.to_string(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum OptSelector<S> {
    Some,
    None,
    Always,
    Never,
    Selector(S),
}

impl<S> OptSelector<S> {
    pub fn always() -> OptSelector<S> {
        Self::Always
    }

    pub fn never() -> OptSelector<S> {
        Self::Never
    }

    pub fn some() -> OptSelector<S> {
        Self::Some
    }

    pub fn none() -> OptSelector<S> {
        Self::None
    }

    pub fn selector(selector: S) -> Self {
        Self::Selector(selector)
    }
}

impl<S, V> PartialEq<Option<V>> for OptSelector<S>
where
    S: PartialEq<V>,
{
    fn eq(&self, opt: &Option<V>) -> bool {
        match self {
            OptSelector::Some => opt.is_some(),
            OptSelector::None => opt.is_none(),
            OptSelector::Always => true,
            OptSelector::Never => false,
            OptSelector::Selector(selector) => {
                if let Option::Some(v) = opt.as_ref() {
                    *selector == *v
                } else {
                    false
                }
            }
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum IdSelector<V>
where
    V: Clone + Eq + PartialEq + Hash,
{
    Always,
    Never,
    Set(HashSet<V>),
}

impl<V> ToString for IdSelector<V>
where
    V: Clone + Eq + PartialEq + Hash + ToString,
{
    fn to_string(&self) -> String {
        match self {
            IdSelector::Always => "*".to_string(),
            IdSelector::Never => "!".to_string(),
            IdSelector::Set(set) => set
                .into_iter()
                .map(|v| v.to_string())
                .collect::<Vec<String>>()
                .join("|"),
        }
    }
}

impl<V> IdSelector<V>
where
    V: Clone + Eq + PartialEq + Hash,
{
    pub fn always() -> IdSelector<V> {
        Self::Always
    }

    pub fn never() -> IdSelector<V> {
        Self::Never
    }

    pub fn single(value: V) -> Self {
        let set = HashSet::from([value]);
        Self::Set(set)
    }

    pub fn or(self, value: V) -> IdSelector<V> {
        match self {
            Self::Always => Self::single(value),
            Self::Never => Self::single(value),
            Self::Set(mut set) => {
                set.insert(value);
                Self::Set(set)
            }
        }
    }
}

impl<V> PartialEq<V> for IdSelector<V>
where
    V: Clone + Eq + PartialEq + Hash,
{
    fn eq(&self, value: &V) -> bool {
        match self {
            IdSelector::Always => true,
            IdSelector::Never => false,
            IdSelector::Set(set) => set.contains(value),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum MatchSelector<S, V>
where
    S: PartialEq<V> + Hash + Eq + PartialEq,
{
    Always,
    Never,
    Set {
        set: HashSet<S>,
        phantom: PhantomData<V>,
    },
}

impl<S, V> PartialEq<V> for MatchSelector<S, V>
where
    S: PartialEq<V> + Hash + Eq + PartialEq,
{
    fn eq(&self, other: &V) -> bool {
        match self {
            MatchSelector::Always => true,
            MatchSelector::Never => false,
            MatchSelector::Set { set, .. } => set.iter().any(|s| s.eq(other)),
        }
    }
}

impl<S, V> MatchSelector<S, V>
where
    S: PartialEq<V> + Hash + Eq + PartialEq,
{
    pub fn single(selector: S) -> Self {
        let set = HashSet::from([selector]);
        let phantom = Default::default();
        Self::Set { set, phantom }
    }

    pub fn always() -> Self {
        MatchSelector::Always
    }

    pub fn never() -> Self {
        MatchSelector::Never
    }

    pub fn or(self, selector: S) -> MatchSelector<S, V> {
        match self {
            MatchSelector::Always => Self::single(selector),
            MatchSelector::Never => Self::single(selector),
            MatchSelector::Set { mut set, phantom } => {
                set.insert(selector);
                Self::Set { set, phantom }
            }
        }
    }
}

impl<S, V> Default for MatchSelector<S, V>
where
    S: PartialEq<V> + Hash + Eq + PartialEq,
{
    fn default() -> Self {
        Self::Always
    }
}

pub struct SelectorSet<S> {
    selectors: Vec<S>,
}

impl<S> SelectorSet<S> {
    pub fn new(selector: S) -> Self {
        Self {
            selectors: vec![selector],
        }
    }
}

impl<S> Default for SelectorSet<S> {
    fn default() -> Self {
        Self { selectors: vec![] }
    }
}

impl<S> Deref for SelectorSet<S> {
    type Target = Vec<S>;

    fn deref(&self) -> &Self::Target {
        &self.selectors
    }
}

impl<S> DerefMut for SelectorSet<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub enum ValuePattern<T> {
    Always,
    Never,
    Pattern(T),
}

impl<T> ValuePattern<T>
where
    T: ToString,
{
    pub fn stringify(self) -> ValuePattern<String> {
        match self {
            ValuePattern::Always => ValuePattern::Always,
            ValuePattern::Never => ValuePattern::Never,
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
            ValuePattern::Always => "*".to_string(),
            ValuePattern::Never => "!".to_string(),
            ValuePattern::Pattern(pattern) => pattern.to_string(),
        }
    }
}

impl<P, T> ValueMatcher<T> for ValuePattern<P>
where
    P: PartialEq<T>,
{
    fn is_match(&self, x: &T) -> Result<(), ()> {
        match self {
            ValuePattern::Always => Ok(()),
            ValuePattern::Never => Err(()),
            ValuePattern::Pattern(pattern) if *pattern == *x => Ok(()),
            _ => Err(()),
        }
    }
}

impl<T> ValuePattern<T> {
    pub fn modify<X, F>(self, mut f: F) -> Result<ValuePattern<X>, ParseErrs>
    where
        F: FnMut(T) -> Result<X, ParseErrs>,
    {
        Ok(match self {
            ValuePattern::Always => ValuePattern::Always,
            ValuePattern::Never => ValuePattern::Never,
            ValuePattern::Pattern(from) => ValuePattern::Pattern(f(from)?),
        })
    }

    pub fn wrap<X>(self, x: X) -> ValuePattern<X> {
        match self {
            ValuePattern::Always => ValuePattern::Always,
            ValuePattern::Never => ValuePattern::Never,
            ValuePattern::Pattern(_) => ValuePattern::Pattern(x),
        }
    }

    pub fn is_match<X>(&self, x: &X) -> Result<(), ()>
    where
        T: ValueMatcher<X>,
    {
        match self {
            ValuePattern::Always => Ok(()),
            ValuePattern::Pattern(exact) => exact.is_match(x),
            ValuePattern::Never => Err(()),
        }
    }

    pub fn is_match_opt<X>(&self, x: &Option<X>) -> Result<(), ()>
    where
        T: ValueMatcher<X>,
    {
        match self {
            ValuePattern::Always => Ok(()),
            ValuePattern::Pattern(exact) => match x {
                None => Err(()),
                Some(x) => self.is_match(x),
            },
            ValuePattern::Never => Err(()),
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

impl PartialEq<String> for RegexMatcher {
    fn eq(&self, other: &String) -> bool {
        self.is_match(other).is_ok()
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
    fn convert(self) -> Result<A, ParseErrs>;
}

pub trait ConvertFrom<A>
where
    Self: Sized,
{
    fn convert_from(a: A) -> Result<Self, ParseErrs>;
}

pub fn uuid() -> loc::Uuid {
    loc::Uuid::from(uuid::Uuid::new_v4()).unwrap()
}

pub fn timestamp() -> Timestamp {
    Timestamp {
        millis: Utc::now().timestamp_millis(),
    }
}

/*
pub fn uuid() -> Uuid {
    unsafe { starlane_uuid() }
}

pub fn timestamp() -> Timestamp {
    unsafe { starlane_timestamp() }
}

 */

pub trait ToResolved<R>
where
    Self: Sized,
{
    fn collapse(self) -> Result<R, ParseErrs> {
        self.to_resolved(&Env::no_point())
    }

    fn to_resolved(self, env: &Env) -> Result<R, ParseErrs>;
}

pub fn log<R, E>(result: Result<R, E>) -> Result<R, E>
where
    E: PrintErr,
{
    match result {
        Ok(r) => Ok(r),
        Err(err) => {
            err.print();
            Err(err)
        }
    }
}

#[cfg(test)]
pub mod test {
    use crate::util::{IdSelector, MatchSelector};

    #[derive(Clone, Eq, PartialEq, Hash)]
    struct TestValue {
        pub name: String,
    }

    impl From<&str> for TestValue {
        fn from(name: &str) -> Self {
            let name = name.to_string();
            Self { name }
        }
    }

    #[derive(Hash, Eq, PartialEq)]
    struct TestSelector {
        pub regex: String,
    }

    impl TestSelector {
        pub fn new(regex: String) -> Self {
            Self { regex }
        }
    }

    impl From<&str> for TestSelector {
        fn from(regex: &str) -> Self {
            Self::new(regex.to_string())
        }
    }

    impl PartialEq<TestValue> for TestSelector {
        fn eq(&self, other: &TestValue) -> bool {
            other.name.matches(&self.regex).count() > 0
        }
    }

    #[test]
    pub fn test() {
        let less: TestValue = "less".into();
        let fae: TestValue = "fae-dra".into();
        let sel1: TestSelector = "fae-x".into();
        let sel2: TestSelector = "fae".into();

        assert!(sel1 != less);
        assert!(sel1 != fae);
        assert!(sel2 != less);
        assert!(sel2 == fae);

        let sel3: MatchSelector<TestSelector, TestValue> = MatchSelector::never();
        assert!(sel3 != less);
        assert!(sel3 != fae);

        let sel4: MatchSelector<TestSelector, TestValue> = MatchSelector::always();

        assert!(sel4 == less);
        assert!(sel4 == fae);

        let sel5: MatchSelector<TestSelector, TestValue> = sel4.or(sel2);

        assert!(sel5 != less);
        assert!(sel5 == fae);

        let sel6: MatchSelector<TestSelector, TestValue> = sel5.or("less".into());

        assert!(sel6.eq(&less));
        assert!(sel6.eq(&fae));
    }

    #[test]
    pub fn test_eq_selector() {
        let less: TestValue = "less".into();
        let fae: TestValue = "fae-dra".into();
        let none: IdSelector<TestValue> = IdSelector::never();
        let any: IdSelector<TestValue> = IdSelector::always();
        let sel: IdSelector<TestValue> = IdSelector::single(fae.clone());

        assert!(none != less);
        assert!(none != fae);

        assert!(any == less);
        assert!(any == fae);

        assert!(sel != less);
        assert!(sel == fae);
    }
}
