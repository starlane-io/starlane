use crate::app::{AppCreate, AppSelect};
use crate::user::UserKey;

pub type OrgKey=u64;

#[derive(Clone,Serialize,Deserialize)]
pub struct OrgCommandWrapper
{
    org: OrgKey,
    user: UserKey,
    command: OrgCommand
}

#[derive(Clone,Serialize,Deserialize)]
pub enum OrgCommand
{
    AppCreate(AppCreate),
    AppSelect(AppSelect),
    Destroy
}
