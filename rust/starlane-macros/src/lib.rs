
use syn::parse::{Parse, ParseStream};
use std::collections::{HashSet, HashMap};
use proc_macro::{Literal};
use quote::{quote, quote_spanned, ToTokens};
use syn::{parse_macro_input, Expr, Ident, Token, Type, Visibility, Item, PathArguments, Meta, NestedMeta, MetaList, MetaNameValue, Lit, ItemEnum};
use std::convert::TryInto;
use quote::__private::{TokenTree, TokenStream};
use nom::error::context;
use nom::sequence::{delimited, tuple};
use nom::bytes::complete::tag;
use nom::character::complete::{alpha1, digit1};

struct ResourceParser {
   pub items: Vec<Item>,
   pub resources: Vec<Resource>,
   pub ident_to_resource: HashMap<String,Resource>
}

#[derive(Clone)]
struct Resource {
    item: Item,
    parents: Vec<Ident>,
    stars: Vec<Ident>,
    key_prefix: Option<String>,
}

impl Resource {
    pub fn new( item: Item) -> Self {
        Self {
            item: item,
            parents: vec![],
            stars: vec![],
            key_prefix: Option::None
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
                                }
                                Meta::List(list) => {

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
                                    if name_value.path.is_ident("prefix") {
                                        if let Lit::Str(str) = name_value.lit {
                                            resource.key_prefix = Option::Some(str.value());
                                        }
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

        let mut ident_to_resource = HashMap::new();
        for resource in &resources {
            ident_to_resource.insert(resource.get_ident().to_string(), resource.clone() );
        }

        Ok(Self{
            items: items,
            resources: resources,
            ident_to_resource
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
pub fn resources(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let parsed = parse_macro_input!(input as ResourceParser );

    let rts: Vec<Ident> = parsed.resources.iter().map(|resource|{
        resource.get_ident()
    }).collect();


    let resource_type_enum_def = quote!{
        pub enum ResourceType {
        #(#rts),*
         }
    };

    let idents : Vec<Ident> = parsed.resources.iter().map(|resource|{
        resource.get_ident()
    }).collect();

    let parents: Vec<Vec<Ident>> = parsed.resources.iter().map(|resource|{
        resource.parents.clone()
    }).collect();

    let stars: Vec<Vec<Ident>> = parsed.resources.iter().map(|resource|{
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


    let resources_def:Vec<Item> =  parsed.resources.clone().iter().map( |resource| {
        resource.item.clone()
    } ).collect();

    println!("resources_def.len() {}",resources_def.len());

    let keys= keys(&parsed);

    let extras = extras();

    proc_macro::TokenStream::from( quote!{
       #extras
       #resource_type_enum_def
       #resource_impl_def
       #(#resources_def)*
       #keys
    })

}

fn keys( parsed: &ResourceParser) -> TokenStream {
    let mut key_stuff = vec![];
    for resource in &parsed.resources {
        let ident = Ident::new(format!("{}Key", resource.get_ident().to_string()).as_str(), resource.get_ident().span());
        let id = Ident::new(format!("{}Id", resource.get_ident().to_string()).as_str(), resource.get_ident().span());


        let key = if resource.parents.is_empty() {
            quote! {
                pub type #id= u64;

                #[derive(Clone,Eq,PartialEq,Hash,Serialize,Deserialize,Debug)]
                pub struct #ident {
                    id: #id
                }

                impl #ident {
                    pub fn parent(&self) -> Option<ResourceKey> {
                        Option::None
                    }

                    pub fn parent_string_bit(&self) -> Option<String> {
                        Option::None
                    }
                }
            }
        } else {
            let parent = if resource.parents.len() > 1 {
                let parent=Ident::new(format!("{}ParentKey", resource.get_ident().to_string()).as_str(), resource.get_ident().span());
                let parents = resource.parents.clone();
                let mut parent_keys = vec!();
                let mut parent_x_parents = vec!();
                //let mut prefixes = vec!();
                for p in &resource.parents{
                    parent_keys.push( Ident::new( format!( "{}Key", p.to_string()).as_str(), p.span() ));
                    parent_x_parents.push( parent.clone() );
//                    prefixes.push( &parsed.ident_to_resource.get(p.to_string().as_str()).unwrap().key_prefix );
                }



                key_stuff.push(quote! {

                    #[derive(Clone,Eq,PartialEq,Hash,Serialize,Deserialize,Debug)]
                    pub enum #parent {
                        #(#parents(#parent_keys)),*
                    }

                    impl for #parent {
                        pub fn string_bit(&self) -> String {
                             match self {
                                #(#parents(key)=>key.string_bit()),*
                             }
                        }

                        pub fn string_prefix(&self) -> String {
                             match self {
                                #(#parents(key)=>key.string_prefix()),*
                             }
                        }

                    }

                    impl Into<ResourceKey> for #parent {
                        pub fn into(self)->ResourceKey {
                            match self {
                              #(#parents(key)=>key.into()),*
                            }
                        }
                    }

                    #(
                       impl Into<#parent_x_parents> for #parent_keys {
                            fn into(self) -> #parent_x_parents {
                                #parent_x_parents::#parents(self)
                            }
                       }
                    )*

                });

                parent
            } else {
                let parent = resource.parents.last().unwrap();
                Ident::new(format!("{}Key", parent.to_string()).as_str(), parent.span())
            };
            quote! {
                pub type #id= u64;
                pub struct #ident {
                    parent: #parent
                    id: #id
                }

                impl #ident {

                    pub fn new( parent: #parent, id: #id ) -> Self {
                        Self {
                            parent: parent,
                            id: id
                        }
                    }


                    pub fn parent(&self) -> Option<ResourceKey> {
                        Option::Some(parent.into())
                    }

                    pub fn parent_string_bit(&self) -> Option<String> {
                        Option::Some(self.parent.string_bit())
                    }

                }
            }
        };
        key_stuff.push(key);

        //COMMON KEY
        let prefix: TokenStream  = resource.key_prefix.as_ref().expect("expected key prefix").clone().parse().unwrap();
        let ident = Ident::new(format!("{}Key", resource.get_ident().to_string()).as_str(), resource.get_ident().span());
        key_stuff.push(quote!{
                 impl #ident {
                    pub fn string_bit(&self) -> String {
                        self.id.to_string()
                    }

                    pub fn string_prefix(&self) -> String {
                       stringify!(#prefix)
                    }


                }

                impl ToString for #ident{
                    pub fn to_string(&self) -> String {
                        let rtn:ResourceKey = self.clone().into();
                        rtn.to_string()
                    }
                }
        });

    }

    let mut idents = vec!();
    let mut idents_keys = vec!();
    for resource in &parsed.resources {
        idents.push( resource.get_ident() );
        idents_keys.push( Ident::new( format!("{}Key",resource.get_ident().to_string()).as_str(), resource.get_ident().span() ) );
    }

    quote!{
        #(#key_stuff)*

        pub enum ResourceKey {
            #(#idents(#idents_keys)),*
        }

        impl ResourceKey {
            pub fn parent(&self)->Option<ResourceKey> {
                match self {
                    #(#idents(key) => key.parent()),*
                }
            }

            pub fn string_bit(&self) -> String {
                 match self {
                    #(#idents(key) => key.string_bit() ),*
                }
            }

            pub fn parent_string_bit(&self) -> String {
                 match self {
                    #(#idents(parent) => format!("{}<{}>",parent.string_bit(),parent.string_prefix()) ),*
                }
            }

            pub fn string_prefix(&self) -> String {
                 match self {
                    #(#idents(key) => key.string_prefix() ),*
                }
            }

            pub fn ancestors(&self) -> Vec<ResourceKey> {
                let mut rtn = vec![];
                let mut ancestor = self;
                while let Option::Some(parent) = ancestor.parent() {
                   rtn.push( parent );
                   ancestor = parent;
                }
                rtn
            }
        }

        impl ToString for ResourceKey {
            pub fn to_string(&self) -> String {
               let mut bits = vec![];
               bits.push(self.string_bit());
               if self.parent().is_some()
               {
                 bits.push(self.parent_string_bit().expected("expected parent to have a string_bit"));
                 let mut ancestor = self;
                 while let Option::Some(ancestor) = ancestor.parent() {
                   if let Option::Some(bit) = ancestor.parent_string_bit()
                   {
                      bits.push(bit);
                   }
                   ancestor = parent;
                 }
               }

               bits.reverse();

               let mut rtn = String::new();
               for i in 0..bits.len() {
                    rtn.push_str(bit.as_str());
                    if( i < bits.len()-1 ) {
                        rtn.push_str('+');
                    }
               }
               rtn.push_str( "<");
               rtn.push_str( self.string_prefix() );
               rtn.push_str( ">");
               rtn
            }
        }
    }
}


fn extras( )  -> TokenStream {

    quote!{

pub struct Error {
    message: String
}

struct KeyBit{
   key_type: Option<String>,
   id: u64
}
impl ResourceKey {

    pub fn parse_key_type(input: &str) -> Res<&str, &str> {
        context(
            "key-type",
            delimited(
                tag("<"),
                 alpha1,
                tag(">"),
            ),
        )(input)
    }

    pub fn parse_key_bit(input: &str) -> Res<&str, KeyBit> {
        context(
            "key-bit",
            tuple( (digit1, parse_key_type) ),
        )(input).map( |(input, (id,key_type))|{
            (input,
            KeyBit{
                key_type: key_type,
                id: id.parse().unwrap() // should not have an error since we know it is a digit
            })
        })
    }
}




    }


}







#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
