use crate::template::{StarTemplate, StarTemplateSelector};

pub struct Constellation {
    pub name: String,
    pub stars: Vec<StarTemplate>,
}

impl Constellation {
    pub fn new(name: String) -> Self {
        Self {
            name: name,
            stars: vec![],
        }
    }

    pub fn select(&self, selector: StarTemplateSelector) -> Option<StarTemplate> {
        for star in &self.stars {
            match &selector {
                StarTemplateSelector::Handle(handle) => {
                    if star.handle == *handle {
                        return Option::Some(star.clone());
                    }
                }
                StarTemplateSelector::Kind(kind) => {
                    if star.kind == *kind {
                        return Option::Some(star.clone());
                    }
                }
            }
        }
        return Option::None;
    }
}

#[derive(Clone, Eq, PartialEq)]
pub enum ConstellationStatus {
    Unknown,
    Assembled,
    Ready,
}

#[cfg(test)]
mod test {
    use tokio::runtime::Runtime;

    #[test]
    pub fn test() {}
}
