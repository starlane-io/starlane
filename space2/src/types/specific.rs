use crate::schema::case::{DomainCase, SkewerCase, Version};


/// [SpecificDef] defines the structure of a [Specific].
/// it is defined with generics in order to promote reuse for implementations such as the
/// [SpecificSelector]
pub struct SpecificDef<Provider,Vendor,Product,Variant,Version> {
    pub provider: Provider,
    pub vendor:  Vendor,
    pub product: Product,
    pub variant: Variant,
    pub version: Version,
}



/// [Specific] is the name for a [SpecificBundle]
pub type Specific = SpecificDef<DomainCase,DomainCase,SkewerCase,SkewerCase,Version>;


pub struct SpecificBundle {

}