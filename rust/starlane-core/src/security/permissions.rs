use core::ops;
use std::str::FromStr;
use nom::combinator::all_consuming;
use crate::security::permissions::parse::permissions;
use crate::error::Error;
use crate::mesh::serde::id::Address;
use crate::mesh::serde::pattern::AddressKindPattern;

pub enum Pattern {
    None,
    Any, // *
    Exact(Address),
}


pub struct Access {
    pub agent: Address,
    pub pattern: AddressKindPattern,
    pub permissions: Permissions,
}

pub struct Grant {
    pub permissions: Permissions,
    pub resource: Address
}

#[derive(Clone,Eq,PartialEq)]
pub struct Permissions {
    pub create: bool,
    pub read: bool,
    pub write: bool,
    pub execute: bool
}

impl Permissions {
    pub fn has( &self, require: Permissions ) -> bool {
        (require.clone() & self) == require
    }
}

impl ops::BitOr<Permissions> for Permissions {
    type Output = Self;

    fn bitor(self, rhs: Permissions) -> Self::Output {
        Self {
            create: self.create | rhs.create,
            read: self.read | rhs.read,
            write: self.write | rhs.write,
            execute: self.execute | rhs.execute,
        }
    }
}

impl ops::BitAnd<&Permissions> for Permissions {
    type Output = Self;

    fn bitand(self, rhs: &Permissions) -> Self::Output {
        Self {
            create: self.create & rhs.create,
            read: self.read & rhs.read,
            write: self.write & rhs.write,
            execute: self.execute & rhs.execute,
        }
    }
}


impl ops::BitAnd<Permissions> for Permissions {
    type Output = Self;

    fn bitand(self, rhs: Permissions) -> Self::Output {
        Self {
            create: self.create & rhs.create,
            read: self.read & rhs.read,
            write: self.write & rhs.write,
            execute: self.execute & rhs.execute,
        }
    }
}

impl ToString for Permissions {
    fn to_string(&self) -> String {
       let create  = if self.create { "C"  } else { "c" };
       let read    = if self.create { "R"  } else { "r" };
       let write   = if self.create { "W"  } else { "w" };
       let execute = if self.create { "X"  } else { "x" };
       format!("{}{}{}{}", create,read,write,execute )
    }
}

impl FromStr for Permissions {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(all_consuming(permissions )(s)?.1)
    }
}


pub mod parse {
    use nom::branch::alt;
    use nom::bytes::complete::tag;
    use crate::security::permissions::Permissions;
    use nom::sequence::tuple;
    use mesh_portal_serde::version::v0_0_1::parse::Res;

    fn create(input: &str) -> Res<&str,bool> {
        alt( (tag("c"),tag("C")))(input).map( |(next,value):(&str,&str) | {
            (next, value.chars().all(char::is_uppercase ) )
        }
    )
    }

    fn read(input: &str) -> Res<&str,bool> {
        alt( (tag("r"),tag("R")))(input).map( |(next,value):(&str,&str) | {
            (next, value.chars().all(char::is_uppercase ) )
        }
        )
    }

    fn write(input: &str) -> Res<&str,bool> {
        alt( (tag("w"),tag("W")))(input).map( |(next,value):(&str,&str) | {
            (next, value.chars().all(char::is_uppercase ) )
        }
        )
    }

    fn execute(input: &str) -> Res<&str,bool> {
        alt( (tag("x"),tag("X")))(input).map( |(next,value):(&str,&str) | {
            (next, value.chars().all(char::is_uppercase ) )
        }
        )
    }

    pub fn permissions( input : &str ) -> Res<&str,Permissions> {
        tuple((create,read,write,execute))(input).map( |(next,(create,read,write,execute))|{
            (next, Permissions{
                create,
                read,
                write,
                execute
            })
        } )
    }




}