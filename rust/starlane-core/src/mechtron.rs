use crate::cache::ArtifactItem;
use crate::config::mechtron::MechtronConfig;

pub struct Mechtron {
    pub config: ArtifactItem<MechtronConfig>
}