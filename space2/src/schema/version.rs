use alloc::string::{String, ToString};
use core::str::FromStr;
use crate::err::ErrStrata;
use crate::schema::case::err::CaseErr;
use crate::schema::case::Version;


impl ToString for Version {
    fn to_string(&self) -> String {
        self.version.to_string()
    }
}

impl TryInto<semver::Version> for Version {
    type Error = ErrStrata;

    fn try_into(self) -> Result<semver::Version, Self::Error> {
        Ok(self.version)
    }
}

impl FromStr for Version {
    type Err = CaseErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let version = semver::Version::from_str(s)?;
        Ok(Self { version })
    }
}



#[cfg(feature="serde")]
mod serde {
    use alloc::string::ToString;
    use core::fmt;
    use core::fmt::Formatter;
    use core::str::FromStr;
    use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
    use serde::de::Visitor;
    use crate::schema::case::Version;

    impl Serialize for Version {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str(self.version.to_string().as_str())
        }
    }

    pub struct VersionVisitor;

    impl<'de> Visitor<'de> for VersionVisitor {
        type Value = Version;

        fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
            formatter.write_str("SemVer version")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            match Version::from_str(v) {
                Ok(version) => Ok(version),
                Err(error) => {
                    //Err(de::Error::custom(error.to_string() ))
                    Err(de::Error::invalid_type(de::Unexpected::Str(v), &self))
                }
            }
        }
    }

    impl<'de> Deserialize<'de> for Version {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_str(VersionVisitor)
        }
    }

}