use std::hash::Hash;

pub trait Cache where Self::Item: Clone, Self::Key: Clone+Hash+Eq+PartialEq+ToString
{
    type Item;

    type Key;

    fn get(self, key: &Self::Key) -> Option<Self::Item>;
}

pub struct ArtifactCache {

}


pub trait Artifact<T> {
  type Item;
}