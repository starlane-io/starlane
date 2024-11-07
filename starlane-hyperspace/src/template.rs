use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use itertools::Itertools;
use starlane_space::selector::KindSelector;
use crate::service::{ServiceSelector, ServiceTemplate};

#[derive(Clone)]
pub struct Templates<T>
where
    T: Clone,
{
    templates: Vec<T>,
}

impl<T> Templates<T>
where
    T: Clone,
{
    pub fn new(templates: Vec<T>) -> Self {
        Self { templates}
    }

    pub fn select_one<S>(&self, selector: &S) -> Option<&T>
    where
        S: PartialEq<T>,
    {
        (&self.templates)
            .into_iter()
            .find_position(|t| *selector == **t)
            .map(|(size, t)| t)
    }
}



impl Templates<ServiceTemplate> {
    pub fn select(&self, selector: &ServiceSelector) -> Vec<ServiceTemplate> {
        todo!()
        //let mut rtn = vec![];
/*        for template in &self.templates {
            if selector.matches(&template.kind) {
                rtn.push(template.clone());
            }
        }
        rtn

 */
    }

    /*
    /// return the first match found
    pub fn select_one(&self, selector: &ServiceSelector) -> Option<ServiceTemplate> {
        self.select(selector).first().cloned()
    }

     */
}

impl<T> Default for Templates<T>
where
    T: Clone,
{
    fn default() -> Self {
        Self {
            templates: Vec::default(),
        }
    }
}

impl Deref for Templates<ServiceTemplate> {
    type Target = Vec<ServiceTemplate>;

    fn deref(&self) -> &Self::Target {
        &self.templates
    }
}

impl DerefMut for Templates<ServiceTemplate> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.templates
    }
}