use starlane_macros::resources;
use std::convert::TryInto;
use std::convert::TryFrom;
use serde::{Serialize,Deserialize};
use std::str::FromStr;

use starlane_core::resource::address::ResourceAddressPart;
use starlane_core::resource::address::ParentAddressPatternRecognizer;
use starlane_core::resource::address::parse_address;

fn main() {
    println!("Hello, world!");
}

pub struct Error{

}

pub enum StarKind{
    Central,
    Space,
    App
}

pub struct Specific{

}

impl FromStr for Specific {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        todo!()
    }
}

impl ToString for Specific{
    fn to_string(&self) -> String {
        todo!()
    }
}

pub struct ResourceKindParts{
    pub resource_type: String,
    pub kind: Option<String>,
    pub specific: Option<Specific>
}

impl FromStr for ResourceKindParts{
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        todo!()
    }
}

impl ToString for ResourceKindParts{
    fn to_string(&self) -> String {
        todo!()
    }
}


resources! {


    #[resource(parents(Root))]
    #[resource(stars(Space))]
    #[resource(prefix="spc")]
    #[resource(ResourceAddressPartKind::SkewerCase)]
    pub struct Space{

    }

    #[resource(parents(Space))]
    #[resource(stars(Space))]
    #[resource(prefix="sub")]
    #[resource(ResourceAddressPartKind::SkewerCase)]
    pub struct SubSpace{

    }

    #[resource(parents(SubSpace))]
    #[resource(stars(App))]
    #[resource(prefix="app")]
    #[resource(ResourceAddressPartKind::SkewerCase)]
    pub struct App{

    }

    #[resource(parents(SubSpace,App))]
    #[resource(stars(Space,App))]
    #[resource(prefix="db")]
    #[resource(ResourceAddressPartKind::SkewerCase)]
    pub struct Database{
    }


    pub enum DatabaseKind{
        Native,
        External(Specific)
    }

}



#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
