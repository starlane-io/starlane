use crate::id::Id;
use std::fmt;
use crate::star::StarKey;

#[derive(Clone)]
pub struct Command
{
    pub from: i32,
    pub gram: ProtoGram
}

#[derive(Clone)]
pub enum ProtoGram
{
    StarLaneProtocolVersion(i32),
    ReportStarKey(StarKey)
}

#[derive(Clone)]
pub enum LaneGram
{
    Proto(ProtoGram),
    Close
}


impl fmt::Display for ProtoGram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            ProtoGram::StarLaneProtocolVersion(version) => format!("StarLaneProtocolVersion({})", version).to_string(),
            ProtoGram::ReportStarKey(id) => format!("ReportStarId({})", id).to_string()
        };
        write!(f, "{}",r)
    }
}