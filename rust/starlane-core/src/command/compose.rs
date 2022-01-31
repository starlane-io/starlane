use mesh_portal_serde::version::latest::entity::request::create::Create;
use mesh_portal_serde::version::latest::entity::request::select::Select;
use mesh_portal_versions::version::v0_0_1::entity::request::create::ProtoCreate;

pub enum ProtoCommand {
    Create(Create),
    Select(Select),
    Publish(ProtoCreate)
}
