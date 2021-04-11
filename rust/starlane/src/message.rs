use crate::id::Id;
use std::fmt;
use crate::star::StarKey;

#[derive(Clone)]
pub struct Command
{
    pub from: i32,
    pub gram: ProtoFrame
}

#[derive(Clone)]
pub enum ProtoFrame
{
    StarLaneProtocolVersion(i32),
    ReportStarKey(StarKey)
}

#[derive(Clone)]
pub enum LaneFrame
{
    Proto(ProtoFrame),
    Close,
    Ping,
    Pong
}


impl fmt::Display for ProtoFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            ProtoFrame::StarLaneProtocolVersion(version) => format!("StarLaneProtocolVersion({})", version).to_string(),
            ProtoFrame::ReportStarKey(id) => format!("ReportStarId({})", id).to_string()
        };
        write!(f, "{}",r)
    }
}