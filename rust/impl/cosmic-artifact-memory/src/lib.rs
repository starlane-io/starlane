

pub struct ArtifactMemory {
  pub map: LruCache<Point,Artifact>
}

impl ArtifactApi for ArtifactMemory {

}






#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
    }
}
