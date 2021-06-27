use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::convert::{TryFrom, TryInto};

use serde::{Deserialize, Serialize};

use crate::core::StarCoreExt;
use crate::error::Error;
use crate::id::Id;
use crate::lane::{ConnectionInfo, ConnectionKind};
use crate::proto::{PlaceholderKernel, ProtoStar, ProtoStarKernel};
use crate::proto::ProtoStarKernel::Mesh;
use crate::star::{ServerKindExt, StarKey, StarKind, StarSubGraphKey, StarTemplateId};
use crate::star::pledge::StarHandle;

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct ConstellationTemplate {
    pub stars: Vec<StarTemplate>,
}

impl ConstellationTemplate {
    pub fn new() -> Self {
        ConstellationTemplate { stars: vec![] }
    }

    pub fn new_basic() -> Self {
        let mut template = ConstellationTemplate { stars: vec![] };

        let mut central = StarTemplate::new(
            StarKeyTemplate::central(),
            StarKind::Central,
            "central".into(),
        );
        let mut mesh = StarTemplate::new(
            StarKeyTemplate::central_geodesic(1),
            StarKind::Mesh,
            "mesh".into()
        );
        let mut space_host = StarTemplate::new(
            StarKeyTemplate::central_geodesic(2),
            StarKind::SpaceHost,
            "space_host".into(),
        );
        let mut app_host = StarTemplate::new(
            StarKeyTemplate::central_geodesic(3),
            StarKind::AppHost,
            "app_host".into(),
        );
        let mut actor_host = StarTemplate::new(
            StarKeyTemplate::central_geodesic(4),
            StarKind::ActorHost,
            "actor_host".into(),
        );
        let mut file_store = StarTemplate::new(
            StarKeyTemplate::central_geodesic(5),
            StarKind::FileStore,
            "file_store".into(),
        );
        let mut web_host = StarTemplate::new(
            StarKeyTemplate::central_geodesic(6),
            StarKind::Web,
            "web_host".into(),
        );
        let mut gateway = StarTemplate::new(
            StarKeyTemplate::central_geodesic(7),
            StarKind::Gateway,
            "gateway".into(),
        );
        let mut artifact_store = StarTemplate::new(
            StarKeyTemplate::central_geodesic(8),
            StarKind::ArtifactStore,
            "artifact_store".into(),
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


    pub fn new_basic_with(mut star_templates: Vec<StarTemplate> ) -> Self {
        let mut standalone = Self::new_basic();
        let mut mesh = standalone.get_star("mesh".into()).cloned().unwrap();
        for mut star_template in star_templates {
            ConstellationTemplate::connect(&mut star_template, &mut mesh);
            standalone.add_star(star_template);
        }

        standalone
    }

    pub fn new_basic_with_database() -> Self {
        let mut database = StarTemplate::new(
            StarKeyTemplate::central_geodesic(10),
            StarKind::Database,
            "database".into(),
        );
        Self::new_basic_with(vec![database])
    }

    pub fn new_client() -> Self {
        let mut template = ConstellationTemplate { stars: vec![] };

        let subgraph_data_key = "client".to_string();

        let mut client = StarTemplate::new(
            StarKeyTemplate::subraph_data_key(subgraph_data_key, 1),
            StarKind::Client,
            "client".into(),
        );

        template.add_star(client);

        template
    }

    pub fn connect(a: &mut StarTemplate, b: &mut StarTemplate) {
        a.add_lane(StarSelector::StarInConstellationTemplate(StarInConstellationTemplateSelector { constellation: ConstellationSelector::Local, star: StarTemplateSelector::Handle(b.handle.clone()) } ));
        b.add_lane(StarSelector::StarInConstellationTemplate(StarInConstellationTemplateSelector { constellation: ConstellationSelector::Local, star: StarTemplateSelector::Handle(a.handle.clone()) } ));
    }

    pub fn add_star(&mut self, star: StarTemplate) {
        self.stars.push(star);
    }

    pub fn get_star(&self, handle: StarTemplateHandle) -> Option<&StarTemplate> {
        for star in &self.stars {
            if star.handle == handle {
                return Option::Some(star);
            }
        }
        Option::None
    }
}

pub type ConstellationTemplateHandle = String;



#[derive(Hash,PartialEq, Eq, Debug, Clone, Serialize, Deserialize,Ord,PartialOrd)]
pub struct StarInConstellationTemplateHandle{
   pub constellation: ConstellationTemplateHandle,
   pub star: StarTemplateHandle
}

impl ToString for StarInConstellationTemplateHandle {
    fn to_string(&self) -> String {
        format!("{}::{}", self.constellation.to_string(), self.star.to_string() )
    }
}

impl StarInConstellationTemplateHandle{
    pub fn new(constellation: ConstellationTemplateHandle, star: StarTemplateHandle ) -> Self {
        Self{
            constellation,
            star
        }
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub enum StarSelector {
    Any,
    StarInConstellationTemplate(StarInConstellationTemplateSelector)
}

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub enum ConstellationSelector {
    Local,
    Named(String),
    AnyInsideMachine(MachineName)
}

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct StarInConstellationTemplateSelector {
    pub constellation: ConstellationSelector,
    pub star: StarTemplateSelector
}

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub enum StarTemplateSelector {
    Handle(StarTemplateHandle),
    Kind(StarKind)
}

#[derive(Hash,PartialEq, Eq, Debug, Clone, Serialize, Deserialize,Ord,PartialOrd)]
pub struct StarTemplateHandle {
    pub name: String,
    pub index: Option<usize>
}

impl StarTemplateHandle {
    pub fn new( name: String ) -> Self {
        Self {
            name,
            index: Option::None
        }
    }

    pub fn with_index( name: String, index: usize ) -> Self {
        Self {
            name,
            index: Option::Some(index)
        }
    }
}

impl ToString for StarTemplateHandle {
    fn to_string(&self) -> String {
        match self.index {
            None => {
                self.name.clone()
            }
            Some(index) => {
                format!("{}[{}]", self.name, index )
            }
        }
    }
}


impl From<&str>  for StarTemplateHandle {
    fn from(name: &str) -> Self {
        Self::new(name.to_string() )
    }
}

pub struct ProtoConstellationLayout {
    pub handles_to_machine: HashMap<StarTemplateHandle,MachineName>,
    pub template: ConstellationTemplate,
    pub machine_to_host: HashMap<MachineName,String>
}

impl ProtoConstellationLayout {
    pub fn new( template: ConstellationTemplate ) -> Self {
        Self {
            handles_to_machine: HashMap::new(),
            template,
            machine_to_host: HashMap::new()
        }
    }

    pub fn set_default_machine( &mut self, machine: MachineName ) {
        for star in &self.template.stars{
            if !self.handles_to_machine.contains_key(&star.handle ) {
                self.handles_to_machine.insert(star.handle.clone(), machine.clone() );
            }
        }
    }

    pub fn set_machine_for_handle( &mut self, machine: MachineName, handle: StarTemplateHandle) {
       self.handles_to_machine.insert(handle.clone(), machine.clone() );
    }
}


pub struct ConstellationLayout {
    pub handles_to_machine: HashMap<StarTemplateHandle,MachineName>,
    pub template: ConstellationTemplate,
    pub machine_to_host_address: HashMap<MachineName,String>
}

impl ConstellationLayout {
    pub fn standalone() -> Result<Self,Error> {
        let mut standalone = ProtoConstellationLayout::new(ConstellationTemplate::new_basic());
        standalone.set_default_machine("server".to_string());
        standalone.try_into()
    }

    pub fn standalone_with_database() -> Result<Self,Error> {
        let mut standalone = ProtoConstellationLayout::new(ConstellationTemplate::new_basic_with_database());
        standalone.set_default_machine("server".to_string());
        standalone.try_into()
    }

    pub fn client() -> Result<Self,Error> {
        let mut standalone = ProtoConstellationLayout::new(ConstellationTemplate::new_client());
        standalone.set_default_machine("client".to_string());
        standalone.try_into()
    }

    pub fn machine_host_address( &self, name: MachineName ) -> String {
        self.machine_to_host_address.get(&name).unwrap_or(&format!("{}:{}",name,crate::starlane::DEFAULT_PORT.clone())).clone()
    }
}

impl TryFrom<ProtoConstellationLayout> for ConstellationLayout{
    type Error = Error;

    fn try_from(value: ProtoConstellationLayout) -> Result<Self, Self::Error> {
        for star in &value.template.stars {
            if !value.handles_to_machine.contains_key(&star.handle ) {
                return Err(format!("missing machine for star handle: {}", star.handle.to_string()).into());
            }
        }
        Ok(Self {
          handles_to_machine: value.handles_to_machine,
          template: value.template,
          machine_to_host_address: value.machine_to_host
        })
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
            subgraph: StarKeySubgraphTemplate::Core,
            index: StarKeyIndexTemplate::Exact(index),
        }
    }

    pub fn central() -> Self {
        StarKeyTemplate {
            subgraph: StarKeySubgraphTemplate::Core,
            index: StarKeyIndexTemplate::Central,
        }
    }

    pub fn subraph_data_key(subgraph_key: String, index: u16) -> Self {
        StarKeyTemplate {
            subgraph: StarKeySubgraphTemplate::SubgraphKey(subgraph_key),
            index: StarKeyIndexTemplate::Exact(index),
        }
    }

    pub fn create(&self) -> Option<StarKey> {
        let subgraph = match &self.subgraph {
            StarKeySubgraphTemplate::Core => {
                vec![]
            }
            StarKeySubgraphTemplate::SubgraphKey(subgraph_data_key) => {
                return Option::None;
            }
            StarKeySubgraphTemplate::Path(path) => path.to_owned(),
        };
        let index = match self.index {
            StarKeyIndexTemplate::Central => 0,
            StarKeyIndexTemplate::Exact(index) => index,
        };

        Option::Some(StarKey::new_with_subgraph(subgraph, index))
    }
}

pub type MachineName = String;

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Serialize, Deserialize)]
pub enum StarKeySubgraphTemplate {
    Core,
    SubgraphKey(MachineName),
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
    pub lanes: Vec<StarSelector>,
    pub kind: StarKind,
    pub handle: StarTemplateHandle,
}

impl StarTemplate {
    pub fn new(key: StarKeyTemplate, kind: StarKind, handle: StarTemplateHandle) -> Self {
        StarTemplate {
            key: key,
            kind: kind,
            lanes: vec![],
            handle: handle,
        }
    }

    pub fn add_lane(&mut self, lane: StarSelector ) {
        self.lanes.push(lane);
    }
}

#[cfg(test)]
mod test {
    use crate::template::ConstellationTemplate;

    #[test]
    pub fn standalone() {
        ConstellationTemplate::new_basic();
    }
}
