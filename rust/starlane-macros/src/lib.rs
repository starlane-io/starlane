
use syn::parse::{Parse, ParseStream};
use std::collections::{HashSet, HashMap};
use proc_macro::{TokenStream, Literal};
use quote::{quote, quote_spanned, ToTokens};
use syn::{parse_macro_input, Expr, Ident, Token, Type, Visibility, Item, PathArguments, Meta, NestedMeta, MetaList, MetaNameValue, Lit, ItemEnum};
use std::convert::TryInto;
use quote::__private::TokenTree;

struct ResourceParser {
   pub items: Vec<Item>,
   pub resources: Vec<Resource>
}

#[derive(Clone)]
struct Resource {
    item: Item,
    parents: Vec<Ident>,
    stars: Vec<Ident>
}

impl Resource {
    pub fn new( item: Item) -> Self {
        Self {
            item: item,
            parents: vec![],
            stars: vec![]
        }
    }

    pub fn get_ident(&self) -> Ident {
        match &self.item {
            Item::Struct(el_struct) => {
                el_struct.ident.clone()
            }
            _ => {
            panic!("expected struct");
            }
        }
    }

    pub fn strip_resource_attributes(&mut self) {
        if let Item::Struct(e) = &mut self.item {
            e.attrs.retain(|attr| {
                if let Option::Some(seg) = attr.path.segments.last() {
                    if seg.ident.to_string() == "resource".to_string() {
                        false
                    } else {
                        true
                    }
                } else {
                    true
                }
            });
        }
    }

}



impl Parse for ResourceParser {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut items: Vec<Item> = vec![];
        let mut resources:Vec<Resource> = vec![];

        while !input.is_empty() {
            let item = input.parse::<Item>()?;
            if let Item::Struct(e) = &item {
                let mut resource = Resource::new(item.clone());
                for attr in &e.attrs {
                    if let Option::Some(seg) = attr.path.segments.last() {

                        if seg.ident.to_string() == "resource".to_string() {
                              let content: Meta = attr.parse_args()?;
                            match content {
                                Meta::Path(path) => {
                                    println!("path: {}", path.segments.last().unwrap().ident.to_string())
                                }
                                Meta::List(list) => {
                                    println!("list : {}", list.path.segments.last().unwrap().ident.to_string());

                                    match list.path.segments.last().unwrap().ident.to_string().as_str() {
                                        "parents" =>  {
                                            resource.parents = to_idents(&list);
                                        }
                                        "stars" =>  {
                                            resource.stars = to_idents(&list);
                                        }
                                        what => {
                                            panic!("unrecognized resource attribute '{}'", what);
                                        }
                                    }
                                }
                                Meta::NameValue(name_value) => {
                                    println!("name_value: {}", name_value.path.segments.last().unwrap().ident.to_string());
                                    match name_value.lit{
                                        Lit::Str(_) => {}
                                        Lit::ByteStr(_) => {}
                                        Lit::Byte(_) => {}
                                        Lit::Char(_) => {}
                                        Lit::Int(_) => {}
                                        Lit::Float(_) => {}
                                        Lit::Bool(b) => {
                                            println!("VALUE: {}",b.value)
                                        }
                                        Lit::Verbatim(_) => {}
                                    }
                                }
                            }

                        }
                    }
                }

                resource.strip_resource_attributes();
                resources.push(resource);
            } else {
                items.push(item );
            }
        }

        Ok(Self{
            items: items,
            resources: resources
        })
    }
}

fn to_idents( list: &MetaList ) -> Vec<Ident> {
    let mut idents = vec![];
        for parent in &list.nested {
            if let NestedMeta::Meta(parent ) = parent {
                idents.push(parent.path().segments.last().unwrap().ident.clone());
            }
        }
    idents
}

#[proc_macro]
pub fn resources(input: TokenStream) -> TokenStream {
    let ResourceParser { items: items,resources: resources } = parse_macro_input!(input as ResourceParser );

    let rts: Vec<Ident> = resources.iter().map(|resource|{
        resource.get_ident()
    }).collect();


    let resource_type_enum_def = quote!{
        pub enum ResourceType {
        #(#rts),*
         }
    };

    let idents : Vec<Ident> = resources.iter().map(|resource|{
        resource.get_ident()
    }).collect();

    let parents: Vec<Vec<Ident>> = resources.iter().map(|resource|{
        resource.parents.clone()
    }).collect();

    let stars: Vec<Vec<Ident>> = resources.iter().map(|resource|{
        resource.stars.clone()
    }).collect();

    let resource_impl_def = quote! {
impl ResourceType {
   pub fn parents(&self) -> Vec<Self> {
      match self {
        #(Self::#idents => vec![#(Self::#parents),*]),*
      }
   }

   pub fn stars(&self) -> Vec<Self> {
      match self {
        #(Self::#idents => vec![#(Self::#stars),*]),*
      }
   }
}
    };


    let resources_def:Vec<Item> =  resources.clone().iter().map( |resource| {
        resource.item.clone()
    } ).collect();

    println!("resources_def.len() {}",resources_def.len());

    TokenStream::from( quote!{
        #resource_type_enum_def
        #resource_impl_def
        #(#resources_def)*
    })



}


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
