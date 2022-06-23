use std::str::FromStr;
use mesh_portal::version::latest::entity::request::create::{PointTemplate, KindTemplate, Template, PointSegFactory};
use mesh_portal::version::latest::id::{Point, RouteSegment};
use mesh_portal::version::latest::messaging::{Message, ReqShell};
use tokio::sync::mpsc;
use crate::message::delivery::Delivery;
use crate::star::StarKey;

lazy_static! {
    pub static ref HYPERUSER: &'static Point = &Point::from_str("hyperspace:users:hyperuser").unwrap();
    pub static ref HYPER_USERBASE: &'static Point = &Point::from_str("hyperspace:users").unwrap();
}

pub struct HyperUser {

}

impl HyperUser {
    pub fn point() -> Point {
        HYPERUSER.clone()
    }

    /*
    pub fn template() -> Template {
        Template {
            point:
            PointTemplate { parent: Point::root_with_route(RouteSegment::Mesh(StarKey::central().to_string())), child_segment_template: PointSegFactory::Exact("hyperuser".to_string()) },
            kind: KindTemplate {
                kind: "User".to_string(),
                sub_kind: None,
                specific: None
            }
        }
    }

     */

    pub fn messenger() -> mpsc::Sender<Message> {
        let (messenger_tx,mut messenger_rx) = mpsc::channel(1024);
        tokio::spawn( async move {
            // right now we basically ignore messages to HyperUser
            while let Option::Some(_) = messenger_rx.recv().await {}
        });
        messenger_tx
    }
}