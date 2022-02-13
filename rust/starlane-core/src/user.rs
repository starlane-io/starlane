use std::str::FromStr;
use mesh_portal::version::latest::entity::request::create::{AddressTemplate, KindTemplate, Template};
use mesh_portal::version::latest::id::Address;
use mesh_portal::version::latest::messaging::{Message, Request};
use mesh_portal_versions::version::v0_0_1::entity::request::create::AddressSegmentTemplate;
use mesh_portal_versions::version::v0_0_1::id::RouteSegment;
use tokio::sync::mpsc;
use crate::message::delivery::Delivery;
use crate::star::StarKey;

pub struct HyperUser {

}

impl HyperUser {
    pub fn address() -> Address {
        Address::from_str(format!("<<{}>>::hyperuser",StarKey::central().to_string()).as_str() ).expect("should be a valid hyperuser address")
    }

    pub fn template() -> Template {
        Template {
            address:
            AddressTemplate { parent: Address::root_with_route(RouteSegment::Mesh(StarKey::central().to_string())), child_segment_template: AddressSegmentTemplate::Exact("hyperuser".to_string()) },
            kind: KindTemplate {
                resource_type: "User".to_string(),
                kind: None,
                specific: None
            }
        }
    }

    pub fn messenger() -> mpsc::Sender<Message> {
        let (messenger_tx,mut messenger_rx) = mpsc::channel(1024);
        tokio::spawn( async move {
            // right now we basically ignore messages to HyperUser
            while let Option::Some(_) = messenger_rx.recv().await {}
        });
        messenger_tx
    }
}