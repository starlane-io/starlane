use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::id::Id;
use crate::lane::{ConnectionInfo, ConnectionKind};
use crate::layout::ConstellationLayout;
use crate::proto::ProtoStarKernel::Mesh;
use crate::proto::{PlaceholderKernel, ProtoStar, ProtoStarKernel};
use crate::star::{ServerKindExt, StarKey, StarKind, StarSubGraphKey};
use crate::core::StarCoreExt;

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct ConstellationTemplate {
    pub stars: Vec<StarTemplate>,
}

impl ConstellationTemplate {
    pub fn new() -> Self {
        ConstellationTemplate { stars: vec![] }
    }

    pub fn new_standalone() -> Self {
        let mut template = ConstellationTemplate { stars: vec![] };

        let mut central = StarTemplate::new(
            StarKeyTemplate::central(),
            StarKind::Central,
            Option::Some("central".to_string()),
        );
        let mut mesh = StarTemplate::new(
            StarKeyTemplate::central_geodesic(1),
            StarKind::Mesh,
            Option::Some("mesh".to_string()),
        );
        let mut space_host = StarTemplate::new(
            StarKeyTemplate::central_geodesic(2),
            StarKind::SpaceHost,
            Option::Some("space_host".to_string()),
        );
        let mut app_host = StarTemplate::new(
            StarKeyTemplate::central_geodesic(3),
            StarKind::AppHost,
            Option::Some("app_host".to_string()),
        );
        let mut actor_host = StarTemplate::new(
            StarKeyTemplate::central_geodesic(4),
            StarKind::ActorHost,
            Option::Some("actor_host".to_string()),
        );
        let mut file_store = StarTemplate::new(
            StarKeyTemplate::central_geodesic(5),
            StarKind::FileStore,
            Option::Some("file_store".to_string()),
        );
        let mut web_host = StarTemplate::new(
            StarKeyTemplate::central_geodesic(6),
            StarKind::Web,
            Option::Some("web_host".to_string()),
        );
        let mut gateway = StarTemplate::new(
            StarKeyTemplate::central_geodesic(7),
            StarKind::Gateway,
            Option::Some("gateway".to_string()),
        );
        let mut artifact_store = StarTemplate::new(
            StarKeyTemplate::central_geodesic(8),
            StarKind::ArtifactStore,
            Option::Some("artifact_store".to_string()),
        );

        ConstellationTemplate::connect(&mut central, &mut mesh);
        ConstellationTemplate::connect(&mut space_host, &mut mesh);
        ConstellationTemplate::connect(&mut app_host, &mut mesh);
        ConstellationTemplate::connect(&mut actor_host, &mut mesh);
        ConstellationTemplate::connect(&mut file_store, &mut mesh);
        ConstellationTemplate::connect(&mut web_host, &mut mesh);
        ConstellationTemplate::connect(&mut gateway, &mut mesh);
        ConstellationTemplate::connect(&mut artifact_store, &mut mesh);

        template.add_star(central);
        template.add_star(mesh);
        template.add_star(space_host);
        template.add_star(app_host);
        template.add_star(actor_host);
        template.add_star(file_store);
        template.add_star(web_host);
        template.add_star(gateway);
        template.add_star(artifact_store);

        template
    }


    pub fn new_standalone_with( mut star_templates: Vec<StarTemplate> ) -> Self {
        let mut standalone = Self::new_standalone();
        let mut mesh = standalone.get_star("mesh".to_string()).cloned().unwrap();
        for mut star_template in star_templates {
            ConstellationTemplate::connect(&mut star_template, &mut mesh);
            standalone.add_star(star_template);
        }

        standalone
    }

    pub fn new_standalone_with_mysql() -> Self {
        let mut database = StarTemplate::new(
            StarKeyTemplate::central_geodesic(10),
            StarKind::Database,
            Option::Some("database".to_string()),
        );
        Self::new_standalone_with(vec![database])
    }

    pub fn new_client() -> Self {
        let mut template = ConstellationTemplate { stars: vec![] };

        let subgraph_data_key = "client".to_string();

        let mut link = StarTemplate::new(
            StarKeyTemplate::subraph_data_key(subgraph_data_key.clone(), 0),
            StarKind::Client,
            Option::Some("link".to_string()),
        );
        let mut client = StarTemplate::new(
            StarKeyTemplate::subraph_data_key(subgraph_data_key, 1),
            StarKind::Client,
            Option::Some("client".to_string()),
        );

        ConstellationTemplate::connect(&mut client, &mut link);

        template.add_star(link);
        template.add_star(client);

        template
    }

    pub fn connect(a: &mut StarTemplate, b: &mut StarTemplate) {
        a.add_lane(LaneEndpointTemplate::new(b.key.clone()));
        b.add_lane(LaneEndpointTemplate::new(a.key.clone()));
    }

    pub fn add_star(&mut self, star: StarTemplate) {
        self.stars.push(star);
    }

    pub fn get_star(&self, handle: String) -> Option<&StarTemplate> {
        for star in &self.stars {
            if let Option::Some(handle) = &star.handle {
                return Option::Some(star);
            }
        }
        Option::None
    }
}

pub struct ConstellationData {
    pub exclude_handles: HashSet<String>,
    pub subgraphs: HashMap<String, Vec<StarSubGraphKey>>,
}

impl ConstellationData {
    pub fn new() -> Self {
        ConstellationData {
            exclude_handles: HashSet::new(),
            subgraphs: HashMap::new(),
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Serialize, Deserialize)]
pub struct StarKeyTemplate {
    pub subgraph: StarKeySubgraphTemplate,
    pub index: StarKeyIndexTemplate,
}

impl StarKeyTemplate {
    pub fn central_geodesic(index: u16) -> Self {
        StarKeyTemplate {
            subgraph: StarKeySubgraphTemplate::Central,
            index: StarKeyIndexTemplate::Exact(index),
        }
    }

    pub fn central() -> Self {
        StarKeyTemplate {
            subgraph: StarKeySubgraphTemplate::Central,
            index: StarKeyIndexTemplate::Central,
        }
    }

    pub fn subraph_data_key(subgraph_key: String, index: u16) -> Self {
        StarKeyTemplate {
            subgraph: StarKeySubgraphTemplate::SubgraphDataKey(subgraph_key),
            index: StarKeyIndexTemplate::Exact(index),
        }
    }

    pub fn create(&self, data: &ConstellationData) -> Result<StarKey, Error> {
        let subgraph = match &self.subgraph {
            StarKeySubgraphTemplate::Central => {
                vec![]
            }
            StarKeySubgraphTemplate::SubgraphDataKey(subgraph_data_key) => {
                if let Option::Some(subgraph) = data.subgraphs.get(subgraph_data_key) {
                    subgraph.clone()
                } else {
                    return Err(
                        format!("could not find subgraph_data_key: {}", subgraph_data_key).into(),
                    );
                }
            }
            StarKeySubgraphTemplate::Path(path) => path.to_owned(),
        };
        let index = match self.index {
            StarKeyIndexTemplate::Central => 0,
            StarKeyIndexTemplate::Exact(index) => index,
        };

        Ok(StarKey::new_with_subgraph(subgraph, index))
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Serialize, Deserialize)]
pub enum StarKeySubgraphTemplate {
    Central,
    SubgraphDataKey(String),
    Path(Vec<StarSubGraphKey>),
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Serialize, Deserialize)]
pub enum StarKeyIndexTemplate {
    Central,
    Exact(u16),
}

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct StarTemplate {
    pub key: StarKeyTemplate,
    pub lanes: Vec<LaneEndpointTemplate>,
    pub kind: StarKind,
    pub handle: Option<String>,
}

impl StarTemplate {
    pub fn new(key: StarKeyTemplate, kind: StarKind, handle: Option<String>) -> Self {
        StarTemplate {
            key: key,
            kind: kind,
            lanes: vec![],
            handle: handle,
        }
    }

    pub fn add_lane(&mut self, lane: LaneEndpointTemplate) {
        self.lanes.push(lane);
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct LaneEndpointTemplate {
    pub star: StarKeyTemplate,
}

impl LaneEndpointTemplate {
    pub fn new(star: StarKeyTemplate) -> Self {
        LaneEndpointTemplate { star: star }
    }
}

#[cfg(test)]
mod test {
    use crate::template::ConstellationTemplate;

    #[test]
    pub fn standalone() {
        ConstellationTemplate::new_standalone();
    }
}
