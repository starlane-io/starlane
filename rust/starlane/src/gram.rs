use crate::id::Id;
use std::fmt;

pub struct Command
{
    pub from: i32,
    pub gram: StarGram
}

pub enum StarGram
{
    StarLaneProtocolVersion(i32),
    ReportStarId(Id),
    RequestUniqueSequence,
    AssignUniqueSequence(i64),
}



impl fmt::Display for StarGram{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            StarGram::StarLaneProtocolVersion(version) => format!("StarLaneProtocolVersion({})",version).to_string(),
            StarGram::ReportStarId(id) => format!("ReportStarId({})",id).to_string(),
            StarGram::RequestUniqueSequence => "RequestUniqueId".to_string(),
            StarGram::AssignUniqueSequence(id)=> format!("AssignUniqueId({})", id).to_string()
        };
        write!(f, "{}",r)
    }
}