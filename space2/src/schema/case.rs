use alloc::string::String;
use core::fmt;
use core::ops::Deref;
use crate::lib::fmt::{Display, Formatter};

#[derive(
    Debug,
    Clone,
    Eq,
    PartialEq,
    Hash,
    derive_name::Name
)]
pub struct CamelCase {
    string: String,
}

impl CamelCase {
    pub fn as_str(&self) -> &str {
        self.string.as_str()
    }
}

impl Display for CamelCase {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.string.as_str())
    }
}

impl Deref for CamelCase {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.string
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct DomainCase {
    string: String,
}





impl Display for DomainCase {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.string.as_str())
    }
}

impl Deref for DomainCase {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.string
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct SkewerCase {
    string: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct VarCase {
    string: String,
}





impl Display for VarCase {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.string.as_str())
    }
}

impl Deref for VarCase {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.string
    }
}





impl Display for SkewerCase {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.string.as_str())
    }
}

impl Deref for SkewerCase {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.string
    }
}




#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Version {
    pub version: semver::Version,
}

impl Deref for Version {
    type Target = semver::Version;

    fn deref(&self) -> &Self::Target {
        &self.version
    }
}


#[cfg(feature="serde")]
mod serde {
    use crate::schema::case::{SkewerCase, VarCase};

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

            let result = result(case::var_case(new_span(string.as_str())));
            match result {
                Ok(var) => Ok(var),
                Err(err) => Err(serde::de::Error::custom(err.to_string())),
            }
        }
    }

    impl Serialize for SkewerCase {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str(self.string.as_str())
        }
    }

    impl<'de> Deserialize<'de> for SkewerCase {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            let string = String::deserialize(deserializer)?;

            let result = result(case::skewer_case(new_span(string.as_str())));
            match result {
                Ok(skewer) => Ok(skewer),
                Err(err) => Err(serde::de::Error::custom(err.to_string())),
            }
        }
    }
}

#[cfg(feature="parse")]
mod parse {
    use core::str::FromStr;
    use crate::schema::case::{CamelCase, DomainCase, SkewerCase, VarCase};
    impl FromStr for SkewerCase {
        type Err = ParseErrs;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            result(all_consuming(case::skewer_case)(new_span(s)))
        }
    }

    impl FromStr for CamelCase {
        type Err = ParseErrs;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            result(all_consuming(case::camel_case)(new_span(s)))
        }
    }

    impl Serialize for DomainCase {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str(self.string.as_str())
        }
    }

    impl<'de> Deserialize<'de> for DomainCase {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            let string = String::deserialize(deserializer)?;

            let result = result(case::domain(new_span(string.as_str())));
            match result {
                Ok(domain) => Ok(domain),
                Err(err) => Err(serde::de::Error::custom(err.to_string())),
            }
        }
    }


    impl FromStr for DomainCase {
        type Err = ParseErrs;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            result(all_consuming(case::domain)(new_span(s)))
        }
    }

    impl FromStr for VarCase {
        type Err = ParseErrs;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            result(all_consuming(case::var_chars)(new_span(s)))?;
            Ok(Self {
                string: s.to_string(),
            })
        }
    }


}
