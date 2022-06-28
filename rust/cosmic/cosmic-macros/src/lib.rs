#![crate_type = "lib"]
#![allow(warnings)]

#[macro_use]
extern crate strum_macros;



use proc_macro::TokenStream;
use std::str::FromStr;
use chrono::{DateTime, Utc};
use nom::combinator::into;
use nom_locate::LocatedSpan;
use proc_macro2::Ident;
use quote::{format_ident, quote, TokenStreamExt, ToTokens};
use quote::__private::ext::RepToTokensExt;
use regex::Regex;
use syn::{parse_macro_input, DataEnum, DataUnion, DeriveInput, FieldsNamed, FieldsUnnamed, ItemStruct, FnArg, Type, PathArguments, GenericArgument, ReturnType, PathSegment, ImplItem, Signature, Attribute, ItemImpl, Visibility, Data};
use syn::__private::TokenStream2;
use syn::parse::{Parse, ParseBuffer, ParseStream};
use syn::parse_quote::ParseQuote;
use syn::spanned::Spanned;
use syn::token::Async;
use mesh_portal::version::latest::config::bind::{BindConfig, RouteSelector};
use mesh_portal::version::latest::id::Uuid;
use mesh_portal::version::latest::messaging::{CmdMethod, MethodPattern};
use mesh_portal::version::latest::parse::{route_attribute, route_attribute_value};
use mesh_portal::version::latest::parse::model::ScopeFilters;
use mesh_portal::version::latest::util::{log, ValuePattern};

#[no_mangle]
pub(crate) extern "C" fn mesh_portal_uuid() -> String
{
    "Uuid".to_string()
}


#[no_mangle]
pub(crate) extern "C" fn mesh_portal_timestamp() -> DateTime<Utc>{
    Utc::now()
}

// this is just to make the user realize he needs to import RequestHandler
#[proc_macro_derive(RequestHandler)]
pub fn request_handler(item: TokenStream) -> TokenStream {
    TokenStream::from(quote!{})
}

#[proc_macro_derive(AsyncRequestHandler)]
pub fn async_request_handler(item: TokenStream) -> TokenStream {
    TokenStream::from(quote!{})
}

#[proc_macro_attribute]
pub fn routes_async(attr: TokenStream, item: TokenStream ) -> TokenStream {
    _routes(attr, item, true)
}

#[proc_macro_attribute]
pub fn routes(attr: TokenStream, item: TokenStream ) -> TokenStream {
    _routes(attr, item, false)
}

fn _routes(attr: TokenStream, item: TokenStream, _async: bool  ) -> TokenStream {

    let item_cp = item.clone();
    let mut impl_item = parse_macro_input!(item_cp as syn::ItemImpl );
//    let mut selectors = vec![];
    let mut static_selectors= vec![];
    let mut static_selector_keys= vec![];
    let mut idents = vec![];
    let impl_name = find_impl_type(&impl_item);

//    let mut output = vec![];

    for item_impl in &impl_item.items {
        if let ImplItem::Method(call) = item_impl {
            if let Some(attr) = find_route_attr(&call.attrs) {
                let internal = attr.tokens.to_token_stream().to_string();
                 idents.push(format_ident!("_{}",call.sig.ident.clone()));
                let selector_ident = format_ident!("__{}_{}__", impl_name, call.sig.ident );
                let route_selector = attr.to_token_stream().to_string();
                static_selector_keys.push(selector_ident.clone());
                let static_selector= quote!{
                    static ref #selector_ident : mesh_portal::version::latest::config::bind::RouteSelector = mesh_portal::version::latest::parse::route_attribute(#route_selector).unwrap();
                };
                static_selectors.push(static_selector );
//println!(" ~~ ROUTE {}", attr.tokens.to_string() );
/*                let route = route( attr.to_token_stream().into(), call.to_token_stream().into() );
                let mut route = parse_macro_input!(route as syn::ImplItem );
                output.push(route);

 */
            } else {
//                output.push( ImplItem::Method(call) );
            }
        } else {
 //           output.push( item_impl );
        }
    }

//    impl_item.items.append( & mut output );

    let self_ty = impl_item.self_ty.clone();

    let input = LocatedSpan::new("blah");

    let attr : TokenStream2 = attr.into();
    let select = if attr.is_empty() {
        quote!{Err(())}
    } else {
        quote!{ #attr.select(request) }
    };

    let select = quote!{Err(())};

    let rtn= if attr.is_empty() {
        quote!{Ok(RespCore::not_found())}
    } else {
        let rtn= match _async {
            true => quote!{
                            #attr.handle(request).await },
            false=> quote!{
                             #attr.handler.handle(request) }
        };
        rtn
    };

    let request_handler = match _async {
        true => Ident::new("AsyncRequestHandler", impl_name.span() ),
        false => Ident::new( "RequestHandler", impl_name.span() )
    };

    let __async = match _async {
        true => quote!{async},
        false => quote!{}
    };

    let __await = match _async {
        true => quote!{.await},
        false => quote!{}
    };

    let __async_trait = match _async {
        true => quote!{#[async_trait]},
        false => quote!{}
    };

    let rtn = quote!{
        #__async_trait
        impl #request_handler for #self_ty {

            #__async fn select( &self, request: & mesh_portal::version::latest::messaging::ReqShell ) -> Result<(),()> {
                 #(
                    if #static_selector_keys.is_match(&request).is_ok() {
                        return Ok(());
                    }
                )*
                #select
            }

            #__async fn handle( &self, request: mesh_portal::version::latest::messaging::RootRequestCtx<mesh_portal::version::latest::messaging::ReqShell>) -> Result<RespCore,MsgErr> {
                #(
                    if #static_selector_keys.is_match(&request.request).is_ok() {
                       return self.#idents( request )#__await;
                    }
                )*
                #rtn
             }
        }

        lazy_static! {
            #( #static_selectors )*
        }

    };


    println!("{}",rtn.to_string());

    TokenStream2::from_iter( vec![rtn,TokenStream2::from(item)] ).into()
}

fn find_impl_type( item_impl: &ItemImpl ) -> Ident{
    if let Type::Path(path) = &*item_impl.self_ty {
        path.path.segments.last().as_ref().unwrap().ident.clone()
    } else {
        panic!("could not get impl name")
    }
}

fn find_route_attr( attrs: &Vec<Attribute> ) -> Option<Attribute> {
    for attr in attrs {
        if attr.path.segments.last().expect("segment").to_token_stream().to_string().as_str() == "route" {
            return Some(attr.clone());
        }
    }
    return None
}

/*
#[proc_macro_attribute]
pub fn route(attr: TokenStream, item: TokenStream ) -> TokenStream {
    item
}

 */

#[proc_macro_attribute]
pub fn route(attr: TokenStream, input: TokenStream ) -> TokenStream {

//  let combined = TokenStream::from_iter( vec![attr,item]);

  let input = parse_macro_input!(input as syn::ImplItemMethod);


    log(route_attribute_value(attr.to_string().as_str())).expect("valid route selector");

//    attr.to_tokens().next();
  // we do this just to test for a valid selector...
  //log(wrapped_route_selector(attr.tokens.to_string().as_str())).expect("properly formatted route selector");

  let params :Vec<FnArg> = input.sig.inputs.clone().into_iter().collect();
  let ctx = params.get(1).expect("route expected RequestCtx<I,M> as first parameter");
  let ctx = messsage_ctx(ctx).expect("route expected RequestCtx<I,M> as first parameter");

  let __await= match input.sig.asyncness {
      None => quote!{},
      Some(_) => quote!{.await}
  };

    let __async= match input.sig.asyncness {
        None => quote!{},
        Some(_) => quote!{async}
    };
  let orig=  input.sig.ident.clone();
  let ident = format_ident!("_{}", input.sig.ident);
  let rtn_type = rtn_type( &input.sig.output );
  let item = ctx.item;

  let expanded = quote! {
      #__async fn #ident( &self, mut ctx: mesh_portal::version::latest::messaging::RootRequestCtx<mesh_portal::version::latest::messaging::ReqShell> ) -> Result<mesh_portal::version::latest::entity::response::RespCore,MsgErr> {
          let mut ctx : mesh_portal::version::latest::messaging::RootRequestCtx<#item> = ctx.transform_input()?;
          let ctx = ctx.push();

          match self.#orig(ctx)#__await {
              Ok(rtn) => Ok(rtn.into()),
              Err(err) => Err(err)
          }
      }

      #input

    };

println!("{}",expanded.to_string());
  TokenStream::from(expanded)
}

pub(crate) enum Item {
    Request,
    RequestCore,
    Payload
}

impl FromStr for Item {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Request" => Ok(Item::Request),
            "RequestCore" => Ok(Item::RequestCore),
            "Payload" => Ok(Item::Payload),
            what => panic!("cannot convert Request to type '{}'", what)
        }
    }
}

pub(crate) struct RequestCtx {
    pub item: GenericArgument,
}

fn messsage_ctx( input: &FnArg )  -> Result<RequestCtx,String>{
   if let FnArg::Typed(i) = input {
            if let Type::Path(path) = &*i.ty {
                if let PathArguments::AngleBracketed(generics) = &path.path.segments.last().expect("expected last segment").arguments
                {
                    let mut args = generics.args.clone();
                    let item = args.pop().expect("expecting a generic for Context Item").into_value();

                    let ctx = RequestCtx {
                        item,
                    };

                    return Ok(ctx);
                }
            }
    }
    Err("Parameter is not a RequestCtx".to_string())
}

fn rtn_type( output: &ReturnType ) -> GenericArgument {

        if let ReturnType::Type(_, t) = output {
            if let Type::Path(path) = &**t {
                if let PathSegment{arguments,..} = path.path.segments.last().expect("expecting Result") {
                    if let PathArguments::AngleBracketed(args) = arguments {
                        return args.args.first().expect("expected Result Ok to be RespCore::from(...) compatible").clone()
                    }
                }
            }
        }

        panic!("route must return Result<R,MsgErr>")
}

struct RouteAttr {
    attribute: Attribute
}

impl Parse for RouteAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut attribute = input.call(Attribute::parse_outer)?;
        Ok(RouteAttr {
            attribute: attribute.remove(0)
        })
    }
}




