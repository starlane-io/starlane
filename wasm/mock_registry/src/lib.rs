wit_bindgen::generate!({
    path: "../../wit/hyperspace.wit",
    world: "registry",
});

struct Registry;
use crate::exports::starlane::hyperspace::registry_api::RegErr;
use crate::exports::starlane::hyperspace::registry_api::Registration;
use crate::starlane::hyperspace::space;

impl crate::exports::starlane::hyperspace::registry_api::Guest for Registry {
    fn assign_host(_: String, _: String) -> Result<(), RegErr> {
        Err(RegErr::NotImplemented)
    }
    fn assign_star(_: String, _: String) -> Result<(), RegErr> {
        Err(RegErr::NotImplemented)
    }
    fn do_delete(_: space::Delete) -> Result<Vec<String>, RegErr> {
        Err(RegErr::NotImplemented)
    }
    fn do_access(_: String, _: String) -> Result<space::Access, RegErr> {
        Err(RegErr::NotImplemented)
    }
    fn do_chown(_: String, _: String, _: String) -> Result<(), RegErr> {
        Err(RegErr::NotImplemented)
    }
    fn do_grant(_: space::AccessGrant) -> Result<(), RegErr> {
        Err(RegErr::NotImplemented)
    }
    fn do_query(_: String, _: space::Query) -> Result<space::QueryResult, RegErr> {
        Err(RegErr::NotImplemented)
    }
    fn do_remove_access(_: i32, _: String) -> Result<(), RegErr> {
        Err(RegErr::NotImplemented)
    }
    fn do_select(_: space::Select) -> Result<Vec<String>, RegErr> {
        Err(RegErr::NotImplemented)
    }
    fn get_properties(_: String) -> Result<Vec<space::Property>, RegErr> {
        Err(RegErr::NotImplemented)
    }
    fn get_record(_: String) -> Result<space::ParticleRecord, RegErr> {
        Err(RegErr::NotImplemented)
    }
    fn list_access(_: Option<String>, _: String) -> Result<Vec<space::IndexedAccessGrant>, RegErr> {
        Err(RegErr::NotImplemented)
    }
    fn register(_: Registration) -> Result<(), RegErr> {
        Err(RegErr::NotImplemented)
    }
    fn scorch() -> Result<(), RegErr> {
        Err(RegErr::NotImplemented)
    }
    fn sequence(_: String) -> Result<u64, RegErr> {
        Err(RegErr::NotImplemented)
    }
    fn set_properties(_: String, _: Vec<space::PropertyMod>) -> Result<(), RegErr> {
        Err(RegErr::NotImplemented)
    }
    fn set_status(_: String, _: space::Status) -> Result<(), RegErr> {
        Err(RegErr::NotImplemented)
    }
}

export!(Registry);

#[cfg(test)]
mod tests {

    #[test]
    fn it_works() {}
}
