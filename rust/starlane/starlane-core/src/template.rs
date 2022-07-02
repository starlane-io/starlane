use std::collections::{HashMap, HashSet};
use std::convert::{TryFrom, TryInto};

use serde::{Deserialize, Serialize};
use cosmic_api::id::{ConstellationName, MachineName, StarKey};

use crate::error::Error;

use crate::proto::ProtoStarKey;

use crate::star::{ServerKindExt, StarKind, StarTemplateId};

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
            StarKind::Central,
            StarHandle::central(),
        );

        let mut relay = StarTemplate::new(
            StarKind::Relay,
            StarHandle::new("mesh"),
        );
        let mut space = StarTemplate::new(
            StarKind::Space,
            StarHandle::new("space")
        );
        let mut app = StarTemplate::new(
            StarKind::App,
            StarHandle::new("app"),
        );
        let mut exe = StarTemplate::new(
            StarKind::Exe,
            StarHandle::new("exe"),
        );
        let mut file_store = StarTemplate::new(
            StarKind::FileStore,
            StarHandle::new("file_store"),
        );
        let mut web = StarTemplate::new(
            StarKind::Web,
            StarHandle::new("web"),
        );

        let mut artifact_store = StarTemplate::new(
            StarKind::ArtifactStore,
            StarHandle::new("artifact_store"),
        );

        ConstellationTemplate::connect(&mut central, &mut relay);
        ConstellationTemplate::connect(&mut space, &mut relay);
        ConstellationTemplate::connect(&mut app, &mut relay);
        ConstellationTemplate::connect(&mut exe, &mut relay);
        ConstellationTemplate::connect(&mut file_store, &mut relay);
        ConstellationTemplate::connect(&mut web, &mut relay);
        ConstellationTemplate::connect(&mut artifact_store, &mut relay);
//        ConstellationTemplate::connect(&mut portal, &mut mesh);

        template.add_star(central);
        template.add_star(relay);
        template.add_star(space);
        template.add_star(app);
        template.add_star(exe);
        template.add_star(file_store);
        template.add_star(web);
        template.add_star(artifact_store);
//        template.add_star(portal );

        template
    }

    /*
    pub fn new_basic_with(star_templates: Vec<StarTemplate>) -> Self {
        let mut standalone = Self::new_basic();
        let mut mesh = standalone.get_star("mesh".into()).cloned().unwrap();
        for mut star_template in star_templates {
            star_template.handle = StarKeyTemplate::central_geodesic(standalone.next_index());
            ConstellationTemplate::connect(&mut star_template, &mut mesh);
            standalone.add_star(star_template);
        }

        standalone
    }
     */

    /*
    pub fn new_basic_with_external() -> Self {
        let external = StarTemplate::new(
            StarKeyTemplate::central_geodesic(10),
            StarKind::K8s,
            "database".into(),
        );
        Self::new_basic_with(vec![external])
    }

     */

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

    pub fn get_star(&self, handle: StarHandle) -> Option<&StarTemplate> {
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
    pub star: StarHandle,
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
    pub fn new(constellation: ConstellationTemplateHandle, star: StarHandle) -> Self {
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
}

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct StarInConstellationTemplateSelector {
    pub constellation: ConstellationSelector,
    pub star: StarTemplateSelector,
}

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub enum StarTemplateSelector {
    Handle(StarHandle),
    Kind(StarKind),
}

#[derive(Hash, PartialEq, Eq, Debug, Clone, Serialize, Deserialize, Ord, PartialOrd)]
pub struct StarHandle {
    pub name: String,
    pub index: u16,
}

impl StarHandle {
    pub fn central() -> Self {
        Self {
            name: "central".to_string(),
            index: 0
        }
    }
    pub fn new<S:ToString>(name: S) -> Self {
        Self {
            name: name.to_string(),
            index: 0
        }
    }

    pub fn with_index<S:ToString>(name: S , index: u16) -> Self {

        Self {
            name: name.to_string(),
            index
        }
    }
}

impl ToString for StarHandle {
    fn to_string(&self) -> String {
        format!("{}[{}]", self.name, self.index)
    }
}

impl From<&str> for StarHandle {
    fn from(name: &str) -> Self {
        Self::new(name.to_string())
    }
}

pub struct ProtoConstellationLayout {
    pub handles_to_machine: HashMap<StarHandle, MachineName>,
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

    pub fn set_machine_for_handle(&mut self, machine: MachineName, handle: StarHandle) {
        self.handles_to_machine
            .insert(handle.clone(), machine.clone());
    }
}

pub struct ConstellationLayout {
    pub handles_to_machine: HashMap<StarHandle, MachineName>,
    pub template: ConstellationTemplate,
    pub machine_to_host_address: HashMap<MachineName, String>,
}

impl ConstellationLayout {

    pub fn standalone() -> Result<Self, Error> {
        let mut standalone = ProtoConstellationLayout::new(ConstellationTemplate::new_basic());
        standalone.set_default_machine("server".to_string());
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
}

impl ConstellationData {
    pub fn new() -> Self {
        ConstellationData {
            exclude_handles: HashSet::new(),
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Serialize, Deserialize)]
pub struct StarKeyTemplate {
    pub subgraph: StarKeySubgraphTemplate,
    pub index: StarKeyConstellationIndexTemplate,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Serialize, Deserialize)]
pub enum StarKeySubgraphTemplate {
    Core,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Serialize, Deserialize)]
pub enum StarKeyConstellationIndexTemplate {
    Central,
    Exact(StarKeyConstellationIndex),
}

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct StarTemplate {
    pub lanes: Vec<LaneTemplate>,
    pub kind: StarKind,
    pub handle: StarHandle,
}

impl StarTemplate {
    pub fn new(kind: StarKind, handle: StarHandle) -> Self {
        StarTemplate {
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
    pub async fn standalone() {
        ConstellationTemplate::new_basic();
    }
}
