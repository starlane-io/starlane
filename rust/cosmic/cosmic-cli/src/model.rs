
use serde::{Serialize,Deserialize};

#[derive(Serialize,Deserialize)]
pub struct LoginResp {
    pub refresh_token:String
}


#[derive(Serialize,Deserialize)]
pub struct AccessTokenResp {
    pub access_token: String
}
