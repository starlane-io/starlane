use crate::id::Id;
use std::sync::Arc;
use crate::star::StarKey;
use crate::error::Error;
use crate::frame::{ResourceMessage, ResourceState};
use serde::{Deserialize, Serialize, Serializer};

#[derive(Clone,Serialize,Deserialize)]
pub struct ResourceKey
{
    pub app_id: Id,
    pub id: Id
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ResourceLocation
{
    pub star: StarKey,
    pub ext: Option<Vec<u8>>
}

impl ResourceLocation
{
    pub fn new( star: StarKey )->Self
    {
        ResourceLocation{
            star: star,
            ext: Option::None
        }
    }

    pub fn new_ext( star: StarKey, ext: Vec<u8>) -> Self
    {
        ResourceLocation{
            star: star,
            ext: Option::Some(ext)
        }
    }
}
