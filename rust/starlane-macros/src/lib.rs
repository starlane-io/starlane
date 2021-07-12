
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
    address_part: Option<Ident>
}

impl Resource {
    pub fn new( item: Item) -> Self {
        Self {
            item: item,
            parents: vec![],
            stars: vec![],
            key_prefix: Option::None,
            address_part: Option::None
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
                                Meta::Path(path) => {
                                    if( path.segments.first().is_some() && path.segments.first().unwrap().ident.to_string().as_str() == "ResourceAddressPartKind" )
                                    {
                                        resource.address_part = Option::Some( path.segments.last().unwrap().ident.clone() );
                                    }
                                }
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

        impl ToString for ResourceType {
            fn to_string(&self) -> String {
                match self {
                    Self::Root => "Root".to_string(),
                    #(Self::#rts => stringify!(#rts).to_string() ),*
                }
            }
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


    let kinds = kinds(&parsed);


    let addresses= addresses(&parsed);
    /*
    proc_macro::TokenStream::from( quote!{
       #extras
       #resource_type_enum_def
       #resource_impl_def
       #(#resources_def)*
       #keys

       #addresses
       #keys
    })
     */

    proc_macro::TokenStream::from( quote!{
       #resource_type_enum_def
       #resource_impl_def
       #(#resources_def)*
       #keys
       #kinds
       #addresses
    })
}
fn addresses( parsed: &ResourceParser ) -> TokenStream {



    let mut addresses = vec![];
    for resource in &parsed.resources{
        let ident = Ident::new(resource.get_ident().to_string().as_str(), resource.get_ident().span());
        let address_ident = Ident::new(format!("{}Address",resource.get_ident().to_string()).as_str(), resource.get_ident().span());
        if resource.parents.is_empty() {
            addresses.push(quote!{
                impl #address_ident {
                    pub fn parent(&self)->Result<Option<ResourceAddress>,Error> {
                        Ok(Option::None)
                    }
                }
            })
        } else if resource.parents.len() == 1 {
            let parent = resource.parents.first().unwrap().clone();
            let parent_address= Ident::new(format!("{}Address",parent.to_string()).as_str(), parent.span());
            addresses.push( quote!{
                 impl #address_ident {
                    pub fn parent(&self)->Result<Option<ResourceAddress>,Error> {
                        let mut parts = self.parts.clone();
                        parts.remove( parts.len() );
                        Ok(Option::Some(#parent_address::try_from(parts)?.into()))
                    }
                }
            } );
        }else  {
            let parents = resource.parents.clone();
            let parent_address: Vec<Ident>= resource.parents.iter().map( |parent|Ident::new(format!("{}Address",parent.to_string()).as_str(), parent.span())).collect();
            addresses.push( quote!{
                 impl #address_ident {
                    pub fn parent(&self)->Result<Option<ResourceAddress>,Error> {
                        unimplemented!()
/*                        let mut parts = self.parts.clone();
                        parts.remove( parts.len() );
                        let matcher = ParentAddressPatternRecognizer::default();
                        let parent_resource_type = matcher.try_from(parts.clone())?;
                        Ok(Option::Some(ResourceAddress::from_parts_and_type(parent_resource_type, parts )?))

 */
                    }
                }
            } );
        }
    }

    let idents: Vec<Ident> = parsed.resources.iter().map(|resource|{
        resource.get_ident()
    }).collect();
    let idents2 = idents.clone();

    let address_idents: Vec<Ident> = parsed.resources.iter().map(|resource|{
        Ident::new( format!("{}Address",resource.get_ident().to_string()).as_str(), resource.get_ident().span() )
    }).collect();

    let address_idents2 = address_idents.clone();
    let address_idents3 = address_idents.clone();
    let address_idents4= address_idents.clone();
    let address_idents5= address_idents.clone();




    quote!{

        pub struct RootAddress{

        }

        impl RootAddress {
           pub fn parent(&self)->Result<Option<ResourceAddress>,Error> {
             Ok(Option::None)
           }
        }

        impl Into<ResourceAddress> for RootAddress{
            fn into(self) -> ResourceAddress {
                ResourceAddress::Root
            }
        }

        impl TryFrom<Vec<ResourceAddressPart>> for RootAddress{
            type Error=Error;
            fn try_from( parts: Vec<ResourceAddressPart> ) -> Result<RootAddress,Self::Error> {
                if parts.is_empty() {
                    Ok(RootAddress{})
                } else {
                    Err("root address should not have remaining parts".into())
                }
            }
        }

        #(pub struct #address_idents {
            parts: Vec<ResourceAddressPart>
        }

        impl #address_idents2 {
           pub fn resource_type(&self) -> ResourceType {
               ResourceType::#idents
           }
        }

          impl Into<ResourceAddress> for #address_idents3 {
                fn into(self) -> ResourceAddress{
                    ResourceAddress::#idents2(self)
                }
           }

          impl TryFrom<Vec<ResourceAddressPart>> for #address_idents4 {
               type Error=Error;

               fn try_from(parts: Vec<ResourceAddressPart> )->Result<Self,Self::Error>{
                    Ok(Self{
                        parts: parts
                    })
               }
            }

         impl FromStr for #address_idents5 {
               type Err=Error;
               fn from_str( s: &str ) -> Result<Self,Self::Err> {
                    let (leftover,parts):(&str,Vec<ResourceAddressPart>) = parse_address_path(s)?;
                    unimplemented!()
               }
         }

        )*

        pub enum ResourceAddress {
            Root,
            #(#idents(#address_idents)),*
        }

        impl FromStr for ResourceAddress {
            type Err=Error;

            fn from_str(s: &str) -> Result<Self,Self::Err>{
                let (leftover,(address,kind)) = parse_address(s)?;
                if leftover.len() > 0 {
                    return Err(format!("cannot process: '{}' from address '{}'", leftover, s).into());
                }
                let kind:ResourceKind = kind.try_into()?;

                match kind.resource_type() {
                    ResourceType::Root => {
                        if !address.is_empty()  {
                            Err("root address must be empty".into())
                        } else {
                            Ok(ResourceAddress::Root)
                        }
                    }
                    #(ResourceType::#idents=>Ok(#address_idents::try_from(address)?.into())),*
                }
            }

        }


        impl ResourceAddress {
            pub fn from_parts_and_type( resource_type:ResourceType, parts: Vec<ResourceAddressPart> ) -> Result<ResourceAddress,Error> {
               match  resource_type {
                    ResourceType::Root => Ok(ResourceAddress::Root),
                    #(ResourceType::#idents => Ok(#address_idents::try_from(parts.clone())?.into()) ),*
               }

            }
        }

        #(#addresses)*
    }

}

fn kinds( parsed: &ResourceParser ) -> TokenStream {
  let mut kind_stuff = vec![];

  let mut resource_kind_enum = String::new();
    resource_kind_enum.push_str("#[derive(Clone,Debug,Eq,PartialEq,Hash,Serialize,Deserialize)]");
    resource_kind_enum.push_str("pub enum ResourceKind {");
    resource_kind_enum.push_str("Root,");

    let mut resource_kind_from_parts = String::new();
    resource_kind_from_parts.push_str("impl TryFrom<ResourceKindParts> for ResourceKind {");
    resource_kind_from_parts.push_str("type Error = Error;");
    resource_kind_from_parts.push_str("fn try_from( parts: ResourceKindParts ) -> Result<Self,Self::Error> { ");
    resource_kind_from_parts.push_str("match parts.resource_type.as_str() { " );

    let mut into_resource_kind_parts = String::new();
    into_resource_kind_parts.push_str( "impl Into<ResourceKindParts> for ResourceKind {");
    into_resource_kind_parts.push_str( "fn into(self) -> ResourceKindParts {");
    into_resource_kind_parts.push_str( "match self {");
    into_resource_kind_parts.push_str( "Self::Root => ResourceKindParts{ resource_type: \"Root\".to_string(), kind:Option::None, specific:Option::None },");


    let mut resource_type = String::new();
    resource_type.push_str( "impl ResourceKind {");
    resource_type.push_str( "pub fn resource_type(self) -> ResourceType {");
    resource_type.push_str( "match self {");
    resource_type.push_str( "Self::Root => ResourceType::Root,");


    for resource in &parsed.resources {
      if let Option::Some(kind) = parsed.kind_for(resource) {
          resource_kind_enum.push_str(format!("{}({}),",resource.get_ident().to_string(),kind.ident.to_string()).as_str() );
          resource_kind_from_parts.push_str( format!("\"{}\"=>Ok({}Kind::try_from(parts)?.into()),",resource.get_ident().to_string(),resource.get_ident().to_string()).as_str(), );
          into_resource_kind_parts.push_str(format!("Self::{}(kind)=>kind.into(),",resource.get_ident().to_string()).as_str() );
          resource_type.push_str(format!("Self::{}(_)=>ResourceType::{},",resource.get_ident().to_string(),resource.get_ident().to_string()).as_str() );
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

          let mut from_parts = String::new();
          from_parts.push_str(format!("impl TryFrom<ResourceKindParts> for {} {{", kind.ident.to_string()).as_str() );
          from_parts.push_str("type Error=Error;");
          from_parts.push_str("fn try_from(parts: ResourceKindParts)->Result<Self,Self::Error>{" );
          from_parts.push_str("match parts.kind.ok_or(\"expected kind\")?.as_str() {");


          let mut into_parts= String::new();
          into_parts.push_str(format!("impl Into<ResourceKindParts> for {} {{", kind.ident.to_string()).as_str() );
          into_parts.push_str("fn into(self)->ResourceKindParts {" );
          into_parts.push_str("match self {");


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


              if!variant.fields.is_empty() {
                  from_parts.push_str(format!("\"{}\" => Ok(Self::{}(parts.specific.ok_or(\"expected a specific\")?)),", variant.ident.to_string(), variant.ident.to_string()).as_str());
              } else {
                  from_parts.push_str(format!("\"{}\" => Ok(Self::{}),", variant.ident.to_string(), variant.ident.to_string()).as_str());
              }

              if!variant.fields.is_empty() {
                  into_parts.push_str(format!("Self::{}(specific)=>ResourceKindParts{{resource_type:\"{}\".to_string(),kind:Option::Some(\"{}\".to_string()),specific:Option::Some(specific)}},",variant.ident.to_string(),resource.get_ident().to_string(),variant.ident.to_string(),).as_str() );
              } else {
                  into_parts.push_str(format!("Self::{}=>ResourceKindParts{{resource_type:\"{}\".to_string(),kind:Option::Some(\"{}\".to_string()),specific:Option::None}},",variant.ident.to_string(),resource.get_ident().to_string(),variant.ident.to_string(),).as_str() );
              }

          }
          has_specific.push_str("}}}" );
          get_specific.push_str("}}}" );
          from_parts.push_str("_ => Err(\"could not match kind\".into())");
          from_parts.push_str("}}}" );
          into_parts.push_str("}}}" );




          let has_specific= syn::parse_str::<Item>( has_specific.as_str() ).unwrap();
          kind_stuff.push(quote!{#has_specific});

          let get_specific= syn::parse_str::<Item>( get_specific.as_str() ).unwrap();
          kind_stuff.push(quote!{#get_specific});

          let from_parts = syn::parse_str::<Item>( from_parts.as_str() ).unwrap();
          kind_stuff.push(quote!{#from_parts});

          let into_parts= syn::parse_str::<Item>( into_parts.as_str() ).unwrap();
          kind_stuff.push(quote!{#into_parts});


          kind_stuff.push(quote!{

            impl #kind_ident{
                pub fn resource_type(&self) -> ResourceType {
                    ResourceType::#resource_ident
                }
            }


            impl Into<ResourceKind> for #kind_ident {
                  fn into(self) -> ResourceKind {
                      ResourceKind::#resource_ident(self)
                  }
              }

            impl TryInto<#kind_ident> for ResourceKind {
                  type Error=Error;
                  fn try_into(self) -> Result<#kind_ident,Self::Error>{
                      if let Self::#resource_ident(rtn) = self {
                          Ok(rtn)
                      } else {
                          Err("could not convert".into())
                      }
                  }
              }

          });

      } else {
          resource_kind_enum.push_str(format!("{},",resource.get_ident().to_string()).as_str() );
          resource_kind_from_parts.push_str( format!("\"{}\"=>Ok(Self::{}),",resource.get_ident().to_string(),resource.get_ident().to_string()).as_str(), );
          into_resource_kind_parts.push_str(format!("Self::{}=>ResourceKindParts{{resource_type:\"{}\".to_string(),kind:Option::None,specific:Option::None}},",resource.get_ident().to_string(),resource.get_ident().to_string()).as_str() );
          resource_type.push_str(format!("Self::{}=>ResourceType::{},",resource.get_ident().to_string(),resource.get_ident().to_string()).as_str() );
      }
  }
    resource_kind_enum.push_str("}");
    resource_kind_from_parts.push_str("what => Err(format!(\"cannot identify ResourceType: '{}'\",what).into())" );
    resource_kind_from_parts.push_str("}}}" );
    into_resource_kind_parts.push_str("}}}" );
    resource_type.push_str("}}}");

    let resource_kind_emum = syn::parse_str::<Item>(resource_kind_enum.as_str()).unwrap();
    kind_stuff.push( quote!{#resource_kind_emum});

    let resource_kind_from_parts = syn::parse_str::<Item>(resource_kind_from_parts.as_str()).unwrap();
    kind_stuff.push( quote!{#resource_kind_from_parts});

    let into_resource_kind_parts= syn::parse_str::<Item>(into_resource_kind_parts.as_str()).unwrap();
    kind_stuff.push( quote!{#into_resource_kind_parts});

    let resource_type= syn::parse_str::<Item>(resource_type.as_str()).unwrap();
    kind_stuff.push( quote!{#resource_type});


    let rtn = quote!{
        #(#kind_stuff)*

        impl ToString for ResourceKind {
            fn to_string(&self) -> String {
                let parts: ResourceKindParts = self.clone().into();
                parts.to_string()
            }
        }


        impl FromStr for ResourceKind {
            type Err=Error;

            fn from_str(s: &str) -> Result<Self,Self::Err>{
                let parts = ResourceKindParts::from_str(s)?;
                Self::try_from(parts)
            }
        }
    };
    rtn
}

fn keys( parsed: &ResourceParser) -> TokenStream {
    let mut key_stuff = vec![];
    for resource in &parsed.resources {
        let ident = Ident::new(format!("{}Key", resource.get_ident().to_string()).as_str(), resource.get_ident().span());
        let id = Ident::new(format!("{}Id", resource.get_ident().to_string()).as_str(), resource.get_ident().span());
        let resource_ident = resource.get_ident();


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
                                #(Self::#parents(key)=>key.string_bit()),*
                             }
                        }

                        pub fn string_prefix(&self) -> String {
                             match self {
                                #(Self::#parents(key)=>key.string_prefix()),*
                             }
                        }

                    }

                    impl Into<ResourceKey> for #parent {
                        fn into(self)->ResourceKey {
                            match self {
                              #(Self::#parents(key)=>key.into()),*
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

            let prefix = Ident::new(resource.key_prefix.as_ref().unwrap().clone().as_str(), resource.get_ident().span() );
            quote! {
                pub type #id= u64;

                #[derive(Clone,Debug,Eq,PartialEq,Hash,Serialize,Deserialize)]
                pub struct #ident {
                    parent: #parent,
                    id: #id
                }

                impl #ident {

                  fn from_keybit( parent: #parent, key_bit: KeyBit ) -> Result<Self,Error> {
                       if key_bit.key_type.as_str() != stringify!(#prefix) {
                          return Err(format!("cannot create '{}' from keybit: '{}'",key_bit.key_type.as_str(),stringify!(#prefix)).into())
                       }
                       Ok(Self {
                         parent: parent,
                         id: key_bit.id as u64
                       })
                    }

                    pub fn new( parent: #parent, id: #id ) -> Self {
                        Self {
                            parent: parent,
                            id: id
                        }
                    }


                    pub fn parent(&self) -> Option<ResourceKey> {
                        Option::Some(self.parent.clone().into())
                    }
                }

            }
        };
        key_stuff.push(key);

        //COMMON KEY
        let prefix: TokenStream  = resource.key_prefix.as_ref().expect("expected key prefix").clone().parse().unwrap();
        let ident = Ident::new(format!("{}Key", resource.get_ident().to_string()).as_str(), resource.get_ident().span());
        let resource = resource.get_ident();
        key_stuff.push(quote!{
            impl #ident {
                pub fn string_bit(&self) -> String {
                    format!("{}{}",stringify!(#prefix),self.id.to_string())
                }

                pub fn string_prefix(&self) -> String {
                   stringify!(#prefix).to_string()
                }


            }

            impl ToString for #ident{
                fn to_string(&self) -> String {
                    let rtn:ResourceKey = self.clone().into();
                    rtn.to_string()
                }
            }

            impl Into<ResourceKey> for #ident {
                fn into(self) -> ResourceKey {
                    ResourceKey::#resource(self)
                }
            }

            impl TryInto<#ident> for ResourceKey {
                type Error=Error;
                fn try_into(self)->Result<#ident,Self::Error> {
                     if let Self::#resource_ident(key) = self {
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
        pub struct RootKey();

        impl RootKey {
            pub fn new()->Self {
                Self()
            }
        }

        impl Into<ResourceKey> for RootKey {
            fn into(self) -> ResourceKey {
                ResourceKey::Root
            }
        }

        impl TryInto<RootKey> for ResourceKey {
            type Error=Error;
            fn try_into(self) -> Result<RootKey,Error> {
                if let Self::Root=self {
                    Ok(RootKey())
                } else {
                    Err("not an instance of a RootKey".into())
                }
            }
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
                    #(Self::#idents(key) => key.parent(),)*
                    Root => Option::None
                }
            }

            pub fn string_bit(&self) -> String {
                 match self {
                    #(Self::#idents(key) => key.string_bit(), )*
                    Root => "".to_string()
                }
            }

            pub fn string_prefix(&self) -> String {
                 match self {
                    #(Self::#idents(key) => key.string_prefix(), )*
                    Root => "".to_string()
                }
            }

            pub fn ancestors(&self) -> Vec<ResourceKey> {
                let mut rtn = vec![];
                let mut ancestor = self.clone();
                while let Option::Some(parent) = ancestor.parent() {
                   rtn.push( parent.clone() );
                   ancestor = parent.clone();
                }
                rtn
            }

            pub fn ancestors_not_root(&self) -> Vec<ResourceKey> {
                let mut rtn = vec![];
                let mut ancestor = self.clone();
                while let Option::Some(parent) = ancestor.parent() {
                   if parent.parent().is_some() {
                      rtn.push( parent.clone() );
                   }
                   ancestor = parent.clone();
                }
                rtn
            }

            fn from_keybit( parent: ResourceKey, key_bit: KeyBit ) -> Result<ResourceKey,Error> {
                match key_bit.key_type.as_str() {

                    #(stringify!(#prefixes) => {
                        Ok(#idents_keys::from_keybit(parent.try_into()?, key_bit )?.into())
                    } ,)*
                    _ => Err("unrecognized keybit".into())
                }
            }

            pub fn to_snake_case(&self) -> String {
                self.to_string().replace(":", "_")
            }

            pub fn to_skewer_case(&self) -> String {
                self.to_string().replace(":", "-")
            }
        }

        impl ToString for ResourceKey {
            fn to_string(&self) -> String {
                let mut ancestors = self.ancestors_not_root();
                ancestors.reverse();
                ancestors.push(self.clone());

                let mut rtn = String::new();
                for i in 0..ancestors.len() {
                    let ancestor = ancestors.get(i).unwrap();
                    rtn.push_str(ancestor.string_bit().as_str());
                    if i < ancestors.len()-1 {
                      rtn.push_str(":");
                    }
                }
                rtn
            }
        }

        impl FromStr for ResourceKey {
            type Err=Error;
            fn from_str( s: &str ) -> Result<Self,Self::Err> {
                let (leftover,keybits) = KeyBits::parse_key_bits(s)?;
                if leftover.len() > 0 {
                    Err(format!("leftover '{}' when trying to parse ResourceKey '{}'",leftover,s).into())
                } else {
                    let mut key = ResourceKey::Root;
                    for bit in keybits {
                        key = Self::from_keybit(key, bit)?;
                    }
                    Ok(key)
                }
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




    }


}







#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
