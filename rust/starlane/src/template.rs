use crate::star::{StarKey, StarKind};
use std::collections::{HashSet, HashMap};
use crate::proto::{ProtoConstellation, PlaceholderKernel, ProtoStar, ProtoStarKernel};
use crate::id::Id;
use crate::proto::ProtoStarKernel::Mesh;
use crate::layout::ConstellationLayout;
use serde::{Serialize,Deserialize};

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct ConstellationTemplate
{
    pub stars: Vec<StarTemplate>
}

impl ConstellationTemplate
{
    pub fn new()->Self
    {
        ConstellationTemplate {
            stars: vec![]
        }
    }

    pub fn new_standalone()->Self
    {
        let mut template = ConstellationTemplate {
            stars: vec![]
        };

        let mut central = StarTemplate::new(StarKeyTemplate::central(), StarKind::Central, Option::Some("central".to_string()) );
        let mut mesh = StarTemplate::new(StarKeyTemplate::central_geodesic(1), StarKind::Mesh, Option::Some("mesh".to_string())  );
        let mut supervisor = StarTemplate::new(StarKeyTemplate::central_geodesic(2), StarKind::Supervisor, Option::Some("supervisor".to_string())  );
        let mut server = StarTemplate::new(StarKeyTemplate::central_geodesic(3), StarKind::Server, Option::Some("server".to_string())  );
        let mut gateway = StarTemplate::new(StarKeyTemplate::central_geodesic(4), StarKind::Gateway, Option::Some("gateway".to_string())  );

        ConstellationTemplate::connect(&mut central, &mut mesh );
        ConstellationTemplate::connect(&mut supervisor, &mut mesh );
        ConstellationTemplate::connect(&mut server , &mut mesh );
        ConstellationTemplate::connect(&mut gateway, &mut mesh );

        template.add_star(central );
        template.add_star(mesh );
        template.add_star(supervisor );
        template.add_star(server );
        template.add_star(gateway );

        template
    }

    pub fn connect(a: &mut StarTemplate, b: &mut StarTemplate)
    {
        a.add_lane(LaneEndpointTemplate::new(b.key.clone()));
        b.add_lane(LaneEndpointTemplate::new(a.key.clone()));
    }

    pub fn add_star(&mut self, star: StarTemplate)
    {
        self.stars.push(star );
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Serialize, Deserialize)]
pub struct StarKeyTemplate
{
    pub constellation: StarKeyConstellationTemplate,
    pub index: StarKeyIndexTemplate
}

impl StarKeyTemplate
{
    pub fn central_geodesic(index:u16) ->Self
    {
        StarKeyTemplate{
            constellation: StarKeyConstellationTemplate::Central,
            index: StarKeyIndexTemplate::Exact(index)
        }
    }

    pub fn central()->Self
    {
        StarKeyTemplate{
            constellation: StarKeyConstellationTemplate::Central,
            index: StarKeyIndexTemplate::Central
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Serialize, Deserialize)]
pub enum StarKeyConstellationTemplate
{
    Central,
    Path(Vec<u8>),
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Serialize, Deserialize)]
pub enum StarKeyIndexTemplate
{
    Central,
    Exact(u16)
}

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct StarTemplate
{
    pub key: StarKeyTemplate,
    pub lanes: Vec<LaneEndpointTemplate>,
    pub kind: StarKind,
    pub handle: Option<String>
}

impl StarTemplate
{
    pub fn new( key: StarKeyTemplate, kind: StarKind, handle: Option<String> )->Self
    {
        StarTemplate {
            key: key,
            kind: kind,
            lanes: vec![],
            handle: handle
        }
    }

    pub fn add_lane( &mut self, lane: LaneEndpointTemplate)
    {
        self.lanes.push( lane );
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct LaneEndpointTemplate
{
    pub star: StarKeyTemplate
}

impl LaneEndpointTemplate
{
    pub fn new( star: StarKeyTemplate )->Self
    {
        LaneEndpointTemplate {
            star: star
        }
    }
}

#[cfg(test)]
mod test
{
    use crate::template::ConstellationTemplate;

    #[test]
    pub fn standalone()
    {
        ConstellationTemplate::new_standalone();
    }
}