use std::collections::{HashMap, HashSet};
use std::convert::{TryFrom, TryInto};

use serde::{Deserialize, Serialize};

use crate::error::Error;

use crate::proto::ProtoStarKey;

use crate::star::{ServerKindExt, StarKey, StarKind, StarSubGraphKey, StarTemplateId};

pub type StarKeyConstellationIndex = u16;

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct ConstellationTemplate {
    pub stars: Vec<StarTemplate>,
}

impl ConstellationTemplate {
    pub fn new() -> Self {
        ConstellationTemplate { stars: vec![] }
    }

    pub fn next_index(&self) -> StarKeyConstellationIndex {
        (self.stars.len() + 1) as StarKeyConstellationIndex
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
            "mesh".into(),
        );
        let mut space = StarTemplate::new(
            StarKeyTemplate::central_geodesic(2),
            StarKind::Space,
            "space".into(),
        );
        let mut app = StarTemplate::new(
            StarKeyTemplate::central_geodesic(3),
            StarKind::App,
            "app".into(),
        );
        let mut mechtron = StarTemplate::new(
            StarKeyTemplate::central_geodesic(4),
            StarKind::Mechtron,
            "mechtron".into(),
        );
        let mut file_store = StarTemplate::new(
            StarKeyTemplate::central_geodesic(5),
            StarKind::FileStore,
            "file_store".into(),
        );
        let mut web = StarTemplate::new(
            StarKeyTemplate::central_geodesic(6),
            StarKind::Web,
            "web".into(),
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
        /*
        let mut portal= StarTemplate::new(
            StarKeyTemplate::central_geodesic(9),
            StarKind::Portal,
            "portal".into(),
        );

         */


        ConstellationTemplate::connect(&mut central, &mut mesh);
        ConstellationTemplate::connect(&mut space, &mut mesh);
        ConstellationTemplate::connect(&mut app, &mut mesh);
        ConstellationTemplate::connect(&mut mechtron, &mut mesh);
        ConstellationTemplate::connect(&mut file_store, &mut mesh);
        ConstellationTemplate::connect(&mut web, &mut mesh);
        ConstellationTemplate::connect(&mut gateway, &mut mesh);
        ConstellationTemplate::connect(&mut artifact_store, &mut mesh);
//        ConstellationTemplate::connect(&mut portal, &mut mesh);

        template.add_star(central);
        template.add_star(mesh);
        template.add_star(space);
        template.add_star(app);
        template.add_star(mechtron);
        template.add_star(file_store);
        template.add_star(web);
        template.add_star(gateway);
        template.add_star(artifact_store);
//        template.add_star(portal );

        template
    }

    pub fn new_basic_with(star_templates: Vec<StarTemplate>) -> Self {
        let mut standalone = Self::new_basic();
        let mut mesh = standalone.get_star("mesh".into()).cloned().unwrap();
        for mut star_template in star_templates {
            star_template.key = StarKeyTemplate::central_geodesic(standalone.next_index());
            ConstellationTemplate::connect(&mut star_template, &mut mesh);
            standalone.add_star(star_template);
        }

        standalone
    }

    pub fn new_basic_with_external() -> Self {
        let external = StarTemplate::new(
            StarKeyTemplate::central_geodesic(10),
            StarKind::K8s,
            "database".into(),
        );
        Self::new_basic_with(vec![external])
    }

    pub fn new_client(gateway_machine: MachineName) -> Self {
        let mut template = ConstellationTemplate { stars: vec![] };

        let _subgraph_data_key = "client".to_string();

        let request_from_constellation =
            ConstellationSelector::AnyWithGatewayInsideMachine(gateway_machine.clone());

        let mut client = StarTemplate::new(
            StarKeyTemplate::subgraph_data_key(request_from_constellation, 1),
            StarKind::Client,
            "client".into(),
        );

        let mut lane = LaneTemplate::new(StarInConstellationTemplateSelector {
            constellation: ConstellationSelector::AnyWithGatewayInsideMachine(gateway_machine),
            star: StarTemplateSelector::Kind(StarKind::Gateway),
        });
        lane.as_key_requestor();

        client.add_lane(lane);

        template.add_star(client);

        template
    }

    pub fn connect(a: &mut StarTemplate, b: &mut StarTemplate) {
        a.add_lane(LaneTemplate::new(StarInConstellationTemplateSelector {
            constellation: ConstellationSelector::Local,
            star: StarTemplateSelector::Handle(b.handle.clone()),
        }));
        b.add_lane(LaneTemplate::new(StarInConstellationTemplateSelector {
            constellation: ConstellationSelector::Local,
            star: StarTemplateSelector::Handle(a.handle.clone()),
        }));
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

#[derive(Hash, PartialEq, Eq, Debug, Clone, Serialize, Deserialize, Ord, PartialOrd)]
pub struct StarInConstellationTemplateHandle {
    pub constellation: ConstellationTemplateHandle,
    pub star: StarTemplateHandle,
}

impl ToString for StarInConstellationTemplateHandle {
    fn to_string(&self) -> String {
        format!(
            "{}::{}",
            self.constellation.to_string(),
            self.star.to_string()
        )
    }
}

impl StarInConstellationTemplateHandle {
    pub fn new(constellation: ConstellationTemplateHandle, star: StarTemplateHandle) -> Self {
        Self {
            constellation,
            star,
        }
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub enum StarSelector {
    Any,
    StarInConstellationTemplate(StarInConstellationTemplateSelector),
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Serialize, Deserialize)]
pub enum ConstellationSelector {
    Local,
    Named(ConstellationName),
    AnyWithGatewayInsideMachine(MachineName),
}

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct StarInConstellationTemplateSelector {
    pub constellation: ConstellationSelector,
    pub star: StarTemplateSelector,
}

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub enum StarTemplateSelector {
    Handle(StarTemplateHandle),
    Kind(StarKind),
}

#[derive(Hash, PartialEq, Eq, Debug, Clone, Serialize, Deserialize, Ord, PartialOrd)]
pub struct StarTemplateHandle {
    pub name: String,
    pub index: Option<usize>,
}

impl StarTemplateHandle {
    pub fn new(name: String) -> Self {
        Self {
            name,
            index: Option::None,
        }
    }

    pub fn with_index(name: String, index: usize) -> Self {
        Self {
            name,
            index: Option::Some(index),
        }
    }
}

impl ToString for StarTemplateHandle {
    fn to_string(&self) -> String {
        match self.index {
            None => self.name.clone(),
            Some(index) => {
                format!("{}[{}]", self.name, index)
            }
        }
    }
}

impl From<&str> for StarTemplateHandle {
    fn from(name: &str) -> Self {
        Self::new(name.to_string())
    }
}

pub struct ProtoConstellationLayout {
    pub handles_to_machine: HashMap<StarTemplateHandle, MachineName>,
    pub template: ConstellationTemplate,
    pub machine_to_host: HashMap<MachineName, String>,
}

impl ProtoConstellationLayout {
    pub fn new(template: ConstellationTemplate) -> Self {
        Self {
            handles_to_machine: HashMap::new(),
            template,
            machine_to_host: HashMap::new(),
        }
    }

    pub fn set_default_machine(&mut self, machine: MachineName) {
        for star in &self.template.stars {
            if !self.handles_to_machine.contains_key(&star.handle) {
                self.handles_to_machine
                    .insert(star.handle.clone(), machine.clone());
            }
        }
    }

    pub fn set_machine_for_handle(&mut self, machine: MachineName, handle: StarTemplateHandle) {
        self.handles_to_machine
            .insert(handle.clone(), machine.clone());
    }
}

pub struct ConstellationLayout {
    pub handles_to_machine: HashMap<StarTemplateHandle, MachineName>,
    pub template: ConstellationTemplate,
    pub machine_to_host_address: HashMap<MachineName, String>,
}

impl ConstellationLayout {

    pub fn standalone() -> Result<Self, Error> {
        let mut standalone = ProtoConstellationLayout::new(ConstellationTemplate::new_basic());
        standalone.set_default_machine("server".to_string());
        standalone.try_into()
    }

    pub fn standalone_with_external() -> Result<Self, Error> {
        let mut standalone =
            ProtoConstellationLayout::new(ConstellationTemplate::new_basic_with_external());
        standalone.set_default_machine("server".to_string());
        standalone.try_into()
    }

    pub fn client(gateway_machine: MachineName) -> Result<Self, Error> {
        let mut standalone =
            ProtoConstellationLayout::new(ConstellationTemplate::new_client(gateway_machine));
        standalone.set_default_machine("client".to_string());
        standalone.try_into()
    }

    pub fn set_machine_host_address(&mut self, machine: MachineName, host_address: String) {
        self.machine_to_host_address.insert(machine, host_address);
    }

    pub fn get_machine_host_adddress(&self, machine: MachineName) -> String {
        self.machine_to_host_address
            .get(&machine)
            .unwrap_or(&format!(
                "starlane-{}:{}",
                machine,
                crate::starlane::DEFAULT_PORT.clone()
            ))
            .clone()
    }
}

impl TryFrom<ProtoConstellationLayout> for ConstellationLayout {
    type Error = Error;

    fn try_from(value: ProtoConstellationLayout) -> Result<Self, Self::Error> {
        for star in &value.template.stars {
            if !value.handles_to_machine.contains_key(&star.handle) {
                return Err(format!(
                    "missing machine for star handle: {}",
                    star.handle.to_string()
                )
                .into());
            }
        }
        Ok(Self {
            handles_to_machine: value.handles_to_machine,
            template: value.template,
            machine_to_host_address: value.machine_to_host,
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
    pub index: StarKeyConstellationIndexTemplate,
}

impl StarKeyTemplate {
    pub fn central_geodesic(index: StarKeyConstellationIndex) -> Self {
        StarKeyTemplate {
            subgraph: StarKeySubgraphTemplate::Core,
            index: StarKeyConstellationIndexTemplate::Exact(index),
        }
    }

    pub fn central() -> Self {
        StarKeyTemplate {
            subgraph: StarKeySubgraphTemplate::Core,
            index: StarKeyConstellationIndexTemplate::Central,
        }
    }

    pub fn subgraph_data_key(
        request_from_constellation: ConstellationSelector,
        index: StarKeyConstellationIndex,
    ) -> Self {
        StarKeyTemplate {
            subgraph: StarKeySubgraphTemplate::RequestStarSubGraphKeyFromConstellation(
                request_from_constellation,
            ),
            index: StarKeyConstellationIndexTemplate::Exact(index),
        }
    }

    pub fn create(&self) -> ProtoStarKey {
        let index = match self.index {
            StarKeyConstellationIndexTemplate::Central => 0,
            StarKeyConstellationIndexTemplate::Exact(index) => index,
        };

        let subgraph = match &self.subgraph {
            StarKeySubgraphTemplate::Core => {
                vec![]
            }
            StarKeySubgraphTemplate::RequestStarSubGraphKeyFromConstellation(_) => {
                return ProtoStarKey::RequestSubKeyExpansion(index)
            }
        };

        ProtoStarKey::Key(StarKey::new_with_subgraph(subgraph, index))
    }
}

pub type MachineName = String;
pub type ConstellationName = String;

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Serialize, Deserialize)]
pub enum StarKeySubgraphTemplate {
    Core,
    RequestStarSubGraphKeyFromConstellation(ConstellationSelector),
    //    Path(Vec<StarSubGraphKey>),
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Serialize, Deserialize)]
pub enum StarKeyConstellationIndexTemplate {
    Central,
    Exact(StarKeyConstellationIndex),
}

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct StarTemplate {
    pub key: StarKeyTemplate,
    pub lanes: Vec<LaneTemplate>,
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

    pub fn add_lane(&mut self, lane: LaneTemplate) -> Result<(), Error> {
        if lane.key_requestor {
            for lane in &self.lanes {
                if lane.key_requestor {
                    error!("a star template can only have one key_requestor lane");
                    return Err("a star template can only have one key_requestor lane".into());
                }
            }
        }

        self.lanes.push(lane);

        Ok(())
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct LaneTemplate {
    pub star_selector: StarInConstellationTemplateSelector,
    pub key_requestor: bool,
}

impl LaneTemplate {
    pub fn new(star_selector: StarInConstellationTemplateSelector) -> Self {
        Self {
            star_selector: star_selector,
            key_requestor: false,
        }
    }

    pub fn as_key_requestor(&mut self) {
        self.key_requestor = true;
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
