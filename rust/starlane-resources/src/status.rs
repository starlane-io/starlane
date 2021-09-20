use serde::{Serialize,Deserialize};

#[derive(Debug,Clone,Serialize,Deserialize,strum_macros::Display,Eq,PartialEq,Hash)]
pub enum Status{
    Unknown,
    Pending,
    Initializing,
    Ready,
    Panic
}