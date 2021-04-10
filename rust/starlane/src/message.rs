use crate::id::Id;
use std::fmt;

pub struct Command
{
    pub from: i32,
    pub gram: ProtoGram
}

pub enum ProtoGram
{
    StarLaneProtocolVersion(i32),
    ReportStarId(Id)
}

pub enum LaneGram
{
    Proto(ProtoGram)
}


impl fmt::Display for ProtoGram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            ProtoGram::StarLaneProtocolVersion(version) => format!("StarLaneProtocolVersion({})", version).to_string(),
            ProtoGram::ReportStarId(id) => format!("ReportStarId({})", id).to_string()
        };
        write!(f, "{}",r)
    }
}