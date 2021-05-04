
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type Labels = HashMap<String,String>;

#[derive(Clone,Serialize,Deserialize)]
pub struct LabelConfig
{
    pub name: String,
    pub index: bool
}

#[derive(Clone,Serialize,Deserialize)]
pub struct UniqueLabelConstraint
{
    pub labels: Vec<String>
}

#[derive(Clone,Serialize,Deserialize)]
pub enum LabelSelectionCriteria
{
    Exact(ExactLabelSelectionCriteria),
    Regex(RegexLabelSelectionCriteria)
}

#[derive(Eq,PartialEq,Clone,Serialize,Deserialize)]
pub struct ExactLabelSelectionCriteria
{
    pub name: String,
    pub value: String
}

impl ExactLabelSelectionCriteria
{
    pub fn new( name: String, value: String )->Self
    {
        ExactLabelSelectionCriteria{
            name: name,
            value: value
        }
    }
}


#[derive(Eq,PartialEq,Clone,Serialize,Deserialize)]
pub struct RegexLabelSelectionCriteria
{
    pub name: String,
    pub pattern: String
}

impl RegexLabelSelectionCriteria
{
    pub fn new(name: String, pattern: String ) ->Self
    {
        RegexLabelSelectionCriteria{
            name: name,
            pattern: pattern
        }
    }
}


