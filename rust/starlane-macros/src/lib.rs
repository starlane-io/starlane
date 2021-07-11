
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
   pub resources: Vec<Resource>,
   pub kinds: Vec<ItemEnum>,
   pub ident_to_resource: HashMap<String,Resource>
}

impl ResourceParser {
    pub fn children_of(&self, parent: Resource ) -> Vec<Resource> {
        let mut rtn = vec!();
        for child in &self.resources {
            for parent_ident in child.parents.clone() {
                if parent.get_ident().to_string() == parent_ident.to_string() {
                    rtn.push(child.clone())
                }
            }
        }
        rtn
    }

    pub fn kind_for(&self, resource: &Resource ) -> Option<ItemEnum> {
        for kind in &self.kinds {
               if kind.ident.to_string() == format!("{}Kind",resource.get_ident().to_string()) {
println!("FOUND KIND MATCH");
                   return Option::Some(kind.clone())
               }
        }
        Option::None
    }
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
        let mut kinds: Vec<ItemEnum> = vec![];
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
                                Meta::Path(path) => {}
                                Meta::List(list) => {
                                    match list.path.segments.last().unwrap().ident.to_string().as_str() {
                                        "parents" => {
                                            resource.parents = to_idents(&list);
                                        }
                                        "stars" => {
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
            } else  if let Item::Enum(e) = &item {
                if !e.ident.to_string().ends_with("Kind") {
                    panic!("only ResourceKinds can be defined here");
                }
println!("ADDING KIND: {}",e.ident.to_string());
                kinds.push(e.clone() );
            } else {
            }
        }



        let mut ident_to_resource = HashMap::new();
        for resource in &resources {
            ident_to_resource.insert(resource.get_ident().to_string(), resource.clone() );
        }



        Ok(Self{
            kinds: kinds,
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
          Root,
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
        Self::Root => vec!(),
        #(Self::#idents => vec![#(Self::#parents),*]),*
      }
   }

   pub fn stars(&self) -> Vec<StarKind> {
      match self {
        Self::Root => vec![StarKind::Central],
        #(Self::#idents => vec![#(StarKind::#stars),*]),*
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

    let kinds = kinds(&parsed);


    /*
    proc_macro::TokenStream::from( quote!{
       #extras
       #resource_type_enum_def
       #resource_impl_def
       #(#resources_def)*
       #keys
    })
     */

    proc_macro::TokenStream::from( quote!{
       #resource_type_enum_def
       #resource_impl_def
       #kinds
    })
}

fn kinds( parsed: &ResourceParser ) -> TokenStream {
  let mut kind_stuff = vec![];


  for resource in &parsed.resources {
      if let Option::Some(kind) = parsed.kind_for(resource) {
          let kind_cp = kind.clone();
          kind_stuff.push(quote!{#kind_cp});

          let kind_ident = kind.ident.clone();
          let resource_ident = resource.get_ident();

          let mut variants = vec![];
          let mut get_specific = String::new();
          get_specific.push_str(format!("impl {} {}", kind.ident.to_string(), "{").as_str() );
          get_specific.push_str("pub fn get_specific(&self)->Option<Specific> {" );
          get_specific.push_str("match self {");

          let mut has_specific = String::new();
          has_specific.push_str(format!("impl {} {}", kind.ident.to_string(), "{").as_str() );
          has_specific.push_str("pub fn has_specific(&self)->bool {" );
          has_specific.push_str("match self {");
          for variant in &kind.variants {
              variants.push( variant.ident.clone() );
              has_specific.push_str(format!("Self::{}", variant.ident.to_string()).as_str());
              if!variant.fields.is_empty()
              {
                  has_specific.push_str("(_)=>true,");
              } else {
                  has_specific.push_str("=>false,");
              }

              get_specific.push_str(format!("Self::{}", variant.ident.to_string()).as_str());
              if!variant.fields.is_empty()
              {
                  get_specific.push_str("(specific)=>Option::Some(specific.clone()),");
              } else {
                  get_specific.push_str("=>Option::None,");
              }
          }
          has_specific.push_str("}}}" );
          get_specific.push_str("}}}" );

          let has_specific= syn::parse_str::<Item>( has_specific.as_str() ).unwrap();
          kind_stuff.push(quote!{#has_specific});

          let get_specific= syn::parse_str::<Item>( get_specific.as_str() ).unwrap();
          kind_stuff.push(quote!{#get_specific});


          kind_stuff.push(quote!{


            impl #kind_ident{
                pub fn resource_type(&self) -> ResourceType {
                    ResourceType::#resource_ident
                }
            }
          });

      }
  }

    let rtn = quote!{
        #(#kind_stuff)*
    };
    rtn
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
               }
            }
        } else {
            let parent = if resource.parents.len() > 1 {
                let parent=Ident::new(format!("{}ParentKey", resource.get_ident().to_string()).as_str(), resource.get_ident().span());
                let parents = resource.parents.clone();
                let parents2 = resource.parents.clone();
                let mut parent_keys = vec!();
                let mut parent_x_parents = vec!();
                for p in &resource.parents{
                    parent_keys.push( Ident::new( format!( "{}Key", p.to_string()).as_str(), p.span() ));
                    parent_x_parents.push( parent.clone() );
                }

                key_stuff.push(quote! {

                    #[derive(Clone,Eq,PartialEq,Hash,Serialize,Deserialize,Debug)]
                    pub enum #parent {
                        #(#parents(#parent_keys)),*
                    }

                    impl #parent {


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

                    impl TryInto<#parent> for ResourceKey {
                        type Error=Error;
                        fn try_into(self)->Result<#parent,Self::Error> {
                            match self {
                              #(Self::#parents(key)=>Ok(#parent::#parents2(key)),)*
                              _ => Err("no match".into())
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

                  fn from_keybit( parent: #parent, key_bit: KeyBit ) -> Result<Self,Error> {
                       if key_bit.key_type.as_str() != strinify!(prefix) {
                          return Err(format!("cannot create '{}' from keybit: '{}'",key_bit.key_type.as_str(),stringify!(prefix)).into())
                       }
                       Ok(Self {
                         parent: parent,
                         id: key_bit.id as id
                       })
                    }

                    pub fn new( parent: #parent, id: #id ) -> Self {
                        Self {
                            parent: parent,
                            id: id
                        }
                    }


                    pub fn parent(&self) -> Option<ResourceKey> {
                        Option::Some(parent.into())
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
                        format!("{}{}",stringify!(#prefix),self.id.to_string())
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


                    impl TryInto<#ident> for ResourceKey {
                        type Error=Error;
                        fn try_into(self)->Result<#ident,Self::Error> {
                             if let Self::#ident(key) = self {
                                  Ok(key)
                             } else {
                                  Err(format!("cannot convert to {}", stringify!(ident)).into())
                             }
                        }
                    }

        });

    }

    let mut idents = vec!();
    let mut idents_keys = vec!();
    let mut prefixes = vec![];
    for resource in &parsed.resources {
        idents.push( resource.get_ident() );
        idents_keys.push( Ident::new( format!("{}Key",resource.get_ident().to_string()).as_str(), resource.get_ident().span() ) );
        prefixes.push(Ident::new(resource.key_prefix.as_ref().unwrap().clone().as_str(), resource.get_ident().span() ) );
    }


    let rtn = quote!{
        #(#key_stuff)*

        #[derive(Clone,Debug,Eq,PartialEq,Hash,Serialize,Deserialize)]
        pub enum RootKey{

        }

        #[derive(Clone,Debug,Eq,PartialEq,Hash,Serialize,Deserialize)]
        pub enum ResourceKey {
            Root,
            #(#idents(#idents_keys)),*
        }

        impl ResourceKey {

            pub fn root() -> Self {
                Self::Root
            }

            pub fn parent(&self)->Option<ResourceKey> {
                match self {
                    #(#idents(key) => key.parent(),)*
                    Root => Option::None
                }
            }

            pub fn string_bit(&self) -> String {
                 match self {
                    #(#idents(key) => key.string_bit(), )*
                    Root => ""
                }
            }

            pub fn string_prefix(&self) -> String {
                 match self {
                    #(#idents(key) => key.string_prefix(), )*
                    Root => ""
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

            pub fn ancestors_not_root(&self) -> Vec<ResourceKey> {
                let mut rtn = vec![];
                let mut ancestor = self;
                while let Option::Some(parent) = ancestor.parent() {
                   if parent.parent().is_some() {
                      rtn.push( parent );
                   }
                   ancestor = parent;
                }
                rtn
            }

            fn from_keybit( parent: ResourceKey, key_bit: KeyBit ) -> Result<ResourceKey,Error> {
                match key_bit.key_type.as_str() {

                    #(stringify!(#prefixes) => {
                        #idents::from_keybit(parent.try_into()?, key_bit )?.into()
                    } ,)*
                    _ => Err("unrecognized keybit".into())
                }
            }
        }

        impl ToString for ResourceKey {
            pub fn to_string(&self) -> String {
                let mut ancestors = self.ancestors_not_root();
                ancestors.reverse();
                ancestors.push(self.clone());

                let mut rtn = String::new();
                for ancestor in ancestors {
                    rtn.push_str(ancestor.string_bit());
                }
                rtn
            }
        }


    };
    rtn

    /*
impl FromStr for ResourceKey {
    type Err=Error;
    fn from_str( s: &str ) -> Result<Self,Self::Err> {
        let key_bits = Self::parse_key_bits(s)?;
        let mut key = Self::root();
        for bit in key_bits {
            key = Self::from_keybit( key, bit )?;
        }
        return Ok(key)
    }
}

 */
}


fn extras( )  -> TokenStream {

    quote!{

pub struct Error {
    message: String
}

struct KeyBit{
   key_type: String,
   id: u64
}

struct KeyBits{
  key_type: String,
  bits: Vec<KeyBit>
}


impl ResourceKey {


     pub fn parse_key_bits(input: &str) -> Res<&str, Vec<KeyBit>> {
        context(
            "key-bits",
             many1( parse_key_bit )
        )(input)
     }

    pub fn parse_key_bit(input: &str) -> Res<&str, KeyBit> {
        context(
            "key-bit",
            tuple( (alpha1, digit1) ),
        )(input).map( |(input, (key_type,id))|{
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
