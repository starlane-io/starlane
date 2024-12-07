#![crate_type = "lib"]
#![allow(warnings)]

use proc_macro::TokenStream;
use std::str::FromStr;

use chrono::Utc;
use proc_macro2::{Ident, Span};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::__private::ext::RepToTokensExt;
use quote::{format_ident, quote, ToTokens};
use syn::__private::TokenStream2;
use syn::parse::{Parse, ParseStream};
use syn::parse_quote::ParseQuote;
use syn::spanned::Spanned;
use syn::{parse_macro_input, Attribute, Data, DeriveInput, Expr, ExprTuple, File, FnArg, GenericArgument, ImplItem, ItemImpl, LitStr, PathArguments, PathSegment, ReturnType, Type, Visibility};

/// This macro will auto implement the `#crt::wave::exchange::asynch::DirectedHandler` trait.
/// In order to finalize the core a `#[handler]` attribute must also be specified
/// above one of the impls.
#[proc_macro_derive(DirectedHandler)]
pub fn directed_handler(item: TokenStream) -> TokenStream {
    TokenStream::from(quote! {})
}

/// This impl attribute creates a `fn handler` which receives directed waves and routes them to the first local method
/// which the route selector matches.
/// To implement:
/// ```
///
/// use #crt::err::SpaceErr;
/// use #crt::hyper::HyperSubstance;
/// use #crt::log::PointLogger;
/// use #crt::substance::Substance;
/// use #crt::substance::Substance::Text;
/// use #crt::wave::core::ReflectedCore;
/// use #crt::exchange::asynch::InCtx;
///
/// #[derive(DirectedHandler)]
/// pub struct MyHandler {
///   logger: PointLogger
/// }
///
/// #[handler]
// /// impl MyHandler {
// ///     /// the route attribute captures an ExtMethod implementing a custom `MyNameIs`
// ///     /// notice that the InCtx will accept any valid substance::Substance
// ///     #[route("Ext<MyNameIs>")]
// ///     pub async fn hello(&self, ctx: InCtx<'_, Text>) -> Result<String, SpaceErr> {
// ///         /// also we can return any Substance in our Reflected wave
// ///         Ok(format!("Hello, {}", ctx.input.to_string()))
// ///     }
// ///
// ///     /// if the function returns nothing then an Empty Ok Reflected will be returned unless
// ///     /// the wave type is `Wave<Signal>`
// ///     #[route("Ext<Bye>")]
// ///     pub async fn bye(&self, ctx: InCtx<'_,()>) {
// ///        self.logger.info("funny that! He left without saying a word!");
///     }
/// }
#[proc_macro_attribute]
pub fn handler(attr: TokenStream, item: TokenStream) -> TokenStream {
    _handler(attr, item, true)
}

#[proc_macro_attribute]
pub fn handler_sync(attr: TokenStream, item: TokenStream) -> TokenStream {
    _handler(attr, item, false)
}

fn _handler(attr: TokenStream, item: TokenStream, _async: bool) -> TokenStream {
    let crt = crt_name();

    let item_cp = item.clone();
    let mut impl_item = parse_macro_input!(item_cp as syn::ItemImpl);
    //    let mut selectors = vec![];
    let mut static_selectors = vec![];
    let mut static_selector_keys = vec![];
    let mut idents = vec![];
    let impl_name = find_impl_type2(&impl_item);

    //    let mut output = vec![];

    for item_impl in &impl_item.items {
        if let ImplItem::Method(call) = item_impl {
            if let Some(attr) = find_route_attr(&call.attrs) {
                let internal = attr.tokens.to_token_stream().to_string();
                idents.push(format_ident!("__{}__route", call.sig.ident.clone()));
                let selector_ident = format_ident!("__{}_{}__", impl_name, call.sig.ident);
                let route_selector = attr.to_token_stream().to_string();
                static_selector_keys.push(selector_ident.clone());
                let static_selector = quote! {
                    static ref #selector_ident : #crt::config::bind::RouteSelector = #crt::parse::route_attribute(#route_selector).unwrap();
                };
                static_selectors.push(static_selector);
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
    let generics = impl_item.generics.clone();
    let where_clause = impl_item.generics.where_clause.clone();

    let attr: TokenStream2 = attr.into();

    let rtn = if attr.is_empty() {
        quote! {Ok(RespCore::not_found())}
    } else {
        let rtn = match _async {
            true => quote! {
            #attr.handle(request).await },
            false => quote! {
            #attr.handler.handle(request) },
        };
        rtn
    };

    let selector = match _async {
        true => quote! {#crt::wave::exchange::asynch::DirectedHandlerSelector},
        false => quote! {#crt::wave::exchange::synch::DirectedHandlerSelector},
    };

    let handler = match _async {
        true => quote! {#crt::wave::exchange::asynch::DirectedHandler},
        false => quote! {#crt::wave::exchange::synch::DirectedHandler},
    };

    let root_ctx = match _async {
        true => quote! {#crt::wave::exchange::asynch::RootInCtx},
        false => quote! {#crt::wave::exchange::synch::RootInCtx},
    };

    let _await = match _async {
        true => quote! {.await},
        false => quote! {},
    };

    let _async_trait = match _async {
        true => quote! {#[async_trait::async_trait]},
        false => quote! {},
    };

    let _async = match _async {
        true => quote! {async},
        false => quote! {},
    };

    let rtn = quote! {
        impl #generics #selector for #self_ty #where_clause{
              fn select<'a>( &self, select: &'a #crt::wave::RecipientSelector<'a>, ) -> Result<&dyn #handler, ()> {
                if select.wave.core().method == #crt::wave::core::Method::Cmd(#crt::wave::core::cmd::CmdMethod::Bounce) {
                    return Ok(self);
                }
                #(
                    if #static_selector_keys.is_match(&select.wave).is_ok() {
                        return Ok(self);
                    }
                )*
                Err(())
              }
        }

        #_async_trait
        impl #generics #handler for #self_ty #where_clause{
            #_async fn handle( &self, ctx: #root_ctx) -> #crt::wave::core::CoreBounce {
                #(
                    if #static_selector_keys.is_match(&ctx.wave).is_ok() {
                       return self.#idents( ctx )#_await;
                    }
                )*
                if ctx.wave.core().method == #crt::wave::core::Method::Cmd(#crt::wave::core::cmd::CmdMethod::Bounce) {
                    return self.bounce(ctx)#_await;
                }
                ctx.not_found().into()
             }
        }

        lazy_static::lazy_static! {
            #( #static_selectors )*
        }

    };

    //    println!("{}", rtn.to_string());

    TokenStream2::from_iter(vec![rtn, TokenStream2::from(item)]).into()
}

fn find_impl_type2(item_impl: &ItemImpl) -> Ident {
    if let Type::Path(path) = &*item_impl.self_ty {
        path.path.segments.last().as_ref().unwrap().ident.clone()
    } else {
        panic!("could not get impl name")
    }
}

fn find_route_attr(attrs: &Vec<Attribute>) -> Option<Attribute> {
    for attr in attrs {
        if attr
            .path
            .segments
            .last()
            .expect("segment")
            .to_token_stream()
            .to_string()
            .as_str()
            == "route"
        {
            return Some(attr.clone());
        }
    }
    return None;
}

/*
#[proc_macro_attribute]
pub fn route(attr: TokenStream, item: TokenStream ) -> TokenStream {
    item
}

 */

#[proc_macro_attribute]
pub fn route(attr: TokenStream, input: TokenStream) -> TokenStream {
    let crt = crt_name();

    let input = parse_macro_input!(input as syn::ImplItemMethod);

    //    log(route_attribute_value(attr.to_string().as_str())).expect("valid route selector");

    //    attr.to_tokens().next();
    // we do this just to mem for a valid selector...
    //log(wrapped_route_selector(attr.tokens.to_string().as_str())).expect("properly formatted route selector");

    let params: Vec<FnArg> = input.sig.inputs.clone().into_iter().collect();
    let ctx = params
        .get(1)
        .expect("route expected InCtx<I,M> as first parameter");
    let ctx = messsage_ctx(ctx).expect("route expected InCtx<I,M> as first parameter");

    let __await = match input.sig.asyncness {
        None => quote! {},
        Some(_) => quote! {.await},
    };

    let root_ctx = match input.sig.asyncness {
        None => quote! {#crt::wave::exchange::synch::RootInCtx},
        Some(_) => quote! {#crt::wave::exchange::asynch::RootInCtx},
    };

    let in_ctx = match input.sig.asyncness {
        None => quote! {#crt::wave::exchange::synch::InCtx},
        Some(_) => quote! {#crt::wave::exchange::asynch::InCtx},
    };

    let __async = match input.sig.asyncness {
        None => quote! {},
        Some(_) => quote! {async},
    };

    let orig = input.sig.ident.clone();
    let ident = format_ident!("__{}__route", input.sig.ident);
    let rtn_type = rtn_type(&input.sig.output);
    let item = ctx.item;

    let expanded = quote! {
      #__async fn #ident( &self, mut ctx: #root_ctx ) -> #crt::wave::core::CoreBounce {
          let ctx: #in_ctx<'_,#item> = match ctx.push::<#item>() {
              Ok(ctx) => ctx,
              Err(err) => {
                    if ctx.wave.is_signal() {
                      return #crt::wave::core::CoreBounce::Absorbed;
                    }
                    else {
                      return #crt::wave::core::CoreBounce::Reflected(err.into());
                    }
              }
          };

          let result = self.#orig(ctx)#__await;
          #rtn_type
      }

      #input

    };

    //println!("{}", expanded.to_string());
    TokenStream::from(expanded)
}

pub(crate) enum Item {
    Request,
    RequestCore,
    Payload,
}

impl FromStr for Item {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Request" => Ok(Item::Request),
            "RequestCore" => Ok(Item::RequestCore),
            "Payload" => Ok(Item::Payload),
            what => panic!("cannot convert Request to type '{}'", what),
        }
    }
}

pub(crate) struct RequestCtx {
    pub item: GenericArgument,
}

fn messsage_ctx(input: &FnArg) -> Result<RequestCtx, String> {
    if let FnArg::Typed(i) = input {
        if let Type::Path(path) = &*i.ty {
            if let PathArguments::AngleBracketed(generics) = &path
                .path
                .segments
                .last()
                .expect("expected last segment")
                .arguments
            {
                let mut args = generics.args.clone();
                let item = args
                    .pop()
                    .expect("expecting a generic for Context Item")
                    .into_value();

                let ctx = RequestCtx { item };

                return Ok(ctx);
            }
        }
    }
    Err("Parameter is not a RequestCtx".to_string())
}

fn rtn_type(output: &ReturnType) -> TokenStream2 {
    let crt = crt_name();
    match output {
        ReturnType::Default => {
            quote! {#crt::wave::Bounce::Absorbed}
        }
        ReturnType::Type(_, path) => {
            if let Type::Path(path) = &**path {
                let PathSegment { ident, arguments } = path.path.segments.last().unwrap();
                match ident.to_string().as_str() {
                    "Result" => {
                        if let PathArguments::AngleBracketed(brackets) = arguments {
                            let arg = brackets.args.first().unwrap();
                            if "Substance" == arg.to_token_stream().to_string().as_str() {
                                quote! {
                                 use #crt::err::CoreReflector;
                                 match result {
                                     Ok(rtn) => #crt::wave::core::CoreBounce::Reflected(starlane::wave::core::ReflectedCore::ok_body(rtn)),
                                     Err(err) => #crt::wave::core::CoreBounce::Reflected(err.as_reflected_core())
                                 }
                                }
                            } else {
                                quote! {
                                 use #crt::err::CoreReflector;
                                 match result {
                                     Ok(rtn) => #crt::wave::core::CoreBounce::Reflected(rtn.into()),
                                     Err(err) => #crt::wave::core::CoreBounce::Reflected(err.as_reflected_core())
                                 }
                                }
                            }
                        } else {
                            panic!("Result without angle brackets")
                        }
                    }
                    "Bounce" => {
                        quote! {
                            let rtn : #crt::wave::core::CoreBounce = result.to_core_bounce();
                            rtn
                        }
                    }
                    "CoreBounce" => {
                        quote! {
                           result
                        }
                    }
                    "ReflectedCore" => {
                        quote! {
                           #crt::wave::core::CoreBounce::Reflected(result)
                        }
                    }
                    what => {
                        panic!("unknown return type: {}", what);
                    }
                }
            } else {
                panic!("expecting a path segment")
            }
        }
    }
}

struct RouteAttr {
    attribute: Attribute,
}

impl Parse for RouteAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut attribute = input.call(Attribute::parse_outer)?;
        Ok(RouteAttr {
            attribute: attribute.remove(0),
        })
    }
}

/// adds a trait to given struct or enum
#[proc_macro_derive(ToSpaceErr)]
pub fn to_space_err(item: TokenStream) -> TokenStream {
    let crt = crt_name();
    let input = parse_macro_input!(item as DeriveInput);
    let ident = &input.ident;
    let rtn = quote! {
       impl #crt::err::ToSpaceErr for #ident {
            fn to_space_err(&self) -> #crt::err::SpaceErr {
                #crt::err::SpaceErr::to_space_err(&self.to_string())
            }
        }
    };
    rtn.into()
}

fn crt_name() -> TokenStream2 {
    quote!(starlane_space)
    /*
    let found_crate = crate_name("starlane_space").expect("my-crate is present in `Cargo.toml`");

    let crt = match found_crate {
        FoundCrate::Itself => quote!(crate),
        FoundCrate::Name(name) => {
            quote!(starlane)
        }
    };
    crt

     */
}

#[cfg(test)]
mod test {
    #[test]
    pub fn test() {}
}



///
/// FORMERLY 'main-primitive-macros'
///
///




/// Takes a given enum (which in turn accepts child enums) and auto generates a `Parent::From` so the child
/// can turn into the parent and a `TryInto<Child> for Parent` so the Parent can attempt to turn into the child.
/// ```
/// #[derive(Autobox)]
/// pub enum Parent {
///   Child(Child)
/// }
///
/// pub enum Child {
///   Variant1,
///   Variant2
/// }
/// ```
/// Will generate something like:
/// ```
/// //impl Autobox for Parent { }
///
/// impl From<Child> for Parent {
///   fn from( child: Child ) -> Self {
///      Self::Child(child)
///   }
/// }
///
/// impl TryInto<Child> for Parent {
///   type Err=ParseErrs;
///
///   fn try_into(self) -> Result<Child,Self::Err> {
///     if let Self::Child(child) = self {
///        Ok(self)
///     } else {
///        Err("err")
///     }
///   }
/// }
/// ```
#[proc_macro_derive(Autobox)]
pub fn autobox(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let ident = &input.ident;

    let mut xforms = vec![];
    if let Data::Enum(data) = &input.data {
        for variant in data.variants.clone() {
            if variant.fields.len() > 1 {
                panic!("derive Transform only works on Enums with single value tuples")
            }

            let variant_ident = variant.ident.clone();

            if variant.fields.len() == 1 {
                let mut i = variant.fields.iter();
                let field = i.next().unwrap().clone();
                let ty = field.ty.clone();
                match ty {
                    Type::Path(path) => {
                        let segment = path.path.segments.last().cloned().unwrap();
                        if segment.ident == format_ident!("{}", "Box") {
                            let ty = match segment.arguments {
                                PathArguments::AngleBracketed(ty) => {
                                    format_ident!("{}", ty.args.to_token_stream().to_string())
                                }
                                _ => panic!("expecting angle brackets"),
                            };

                            let ty_str = ty.to_string();

                            xforms.push(quote! {
                                impl TryInto<#ty> for #ident {
                                    type Error=ParseErrs;

                                    fn try_into(self) -> Result<#ty,Self::Error> {
                                        match self {
                                        Self::#variant_ident(val) => Ok(*val),
                                        _ => Err(ParseErrs::new(format!("expected {}",#ty_str)))
                                        }
                                    }
                                }


                                impl From<#ty> for #ident {
                                    fn from(f: #ty) -> #ident {
                                    #ident::#variant_ident(Box::new(f))
                                }
                            }
                                    });
                        } else {
                            let ty = segment.ident;
                            let ty_str = ty.to_token_stream().to_string();
                            xforms.push(quote! {
                                impl TryInto<#ty> for #ident {
                                    type Error=ParseErrs;

                                    fn try_into(self) -> Result<#ty,Self::Error> {
                                        match self {
                                            Self::#variant_ident(val) => Ok(val),
                                            _ => Err(ParseErrs::new(format!("expected {}",#ty_str)))
                                        }
                                    }
                                }


                                impl From<#ty> for #ident {
                                    fn from(f: #ty) -> #ident {
                                        #ident::#variant_ident(f)
                                    }
                                }
                            });
                        }
                    }
                    _ => {
                        panic!("TransformVariants can only handle Path types")
                    }
                }
            }
        }
    } else {
        panic!("derive Transform only works on Enums")
    }

    let rtn = quote! { #(#xforms)* };

    rtn.into()
}

#[proc_macro_derive(ToSubstance)]
pub fn to_substance(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let ident = &input.ident;

    let mut xforms = vec![];
    if let Data::Enum(data) = &input.data {
        for variant in data.variants.clone() {
            if variant.fields.len() > 1 {
                panic!("derive Transform only works on Enums with single value tuples")
            }

            let variant_ident = variant.ident.clone();

            if variant.fields.len() == 1 {
                let mut i = variant.fields.iter();
                let field = i.next().unwrap().clone();
                let ty = field.ty.clone();
                match ty {
                    Type::Path(path) => {
                        let segment = path.path.segments.last().cloned().unwrap();
                        if segment.ident == format_ident!("{}", "Box") {
                            let ty = match segment.arguments {
                                PathArguments::AngleBracketed(ty) => {
                                    format_ident!("{}", ty.args.to_token_stream().to_string())
                                }
                                _ => panic!("expecting angle brackets"),
                            };

                            let ty_str = ty.to_string();

                            xforms.push(quote! {
                            impl ToSubstance<#ty> for #ident {
                                fn to_substance(self) -> Result<#ty,ParseErrs> {
                                    match self {
                                    Self::#variant_ident(val) => Ok(*val),
                                    _ => Err(ParseErrs::new(format!("expected {}",#ty_str)))
                                    }
                                }

                                fn to_substance_ref(&self) -> Result<&#ty,ParseErrs> {
                                    match self {
                                    Self::#variant_ident(val) => Ok(val.as_ref()),
                                    _ => Err(ParseErrs::new(format!("expected {}",#ty_str)))
                                    }
                                }
                            }

                                });
                        } else {
                            let ty = segment.ident;
                            let ty_str = ty.to_token_stream().to_string();
                            xforms.push(quote! {
                            impl ToSubstance<#ty> for #ident {
                                fn to_substance(self) -> Result<#ty,ParseErrs> {
                                    match self {
                                    Self::#variant_ident(val) => Ok(val),
                                    _ => Err(ParseErrs::new(format!("expected {}",#ty_str)))
                                    }
                                }
                                 fn to_substance_ref(&self) -> Result<&#ty,ParseErrs> {
                                    match self {
                                    Self::#variant_ident(val) => Ok(val),
                                    _ => Err(ParseErrs::new(format!("expected {}",#ty_str)))
                                    }
                                }
                            }

                            });
                        }
                    }
                    _ => {
                        panic!("ToSubstance can only handle Path types")
                    }
                }
            } else {
                xforms.push(quote! {
                impl ToSubstance<()> for #ident {
                    fn to_substance(self) -> Result<(),ParseErrs> {
                        match self {
                        Self::#variant_ident => Ok(()),
                        _ => Err(ParseErrs::new(format!("expected Empty")))
                        }
                    }
                     fn to_substance_ref(&self) -> Result<&(),ParseErrs> {
                        match self {
                        Self::#variant_ident => Ok(&()),
                        _ => Err(ParseErrs::new(format!("expected Empty")))
                        }
                    }
                }

                });
            }
        }
    } else {
        panic!("derive ToSubstance only works on Enums")
    }

    let rtn = quote! { #(#xforms)* };

    rtn.into()
}

/*
#[proc_macro_derive(MechErr)]
pub fn mech_err(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let ident = &input.ident;

    let from = vec![
        quote!(Box<bincode::ErrorKind>),
        quote!(mechtron::err::MembraneErr),
        quote!(main::err::ParseErrs),
        quote!(String),
        quote!(&'static str),
        quote!(mechtron::err::GuestErr),
        quote!(std::string::FromUtf8Error),
    ];

    let rtn = quote! {

        impl MechErr for #ident {
            fn to_uni_err(self) -> main::err::{
               main::err::SpaceErr::server_error(self.to_string())
            }
        }

        impl From<#ident> for mechtron::err::GuestErr{
            fn from(e: #ident) -> Self {
                        mechtron::err::GuestErr {
                            message: e.to_string()
                        }
            }
        }

        impl main::err::CoreReflector for #ident {
                fn as_reflected_core(self) -> main::wave::core::ReflectedCore {
                   main::wave::core::ReflectedCore{
                        headers: Default::default(),
                        status: main::wave::core::http2::StatusCode::from_u16(500u16).unwrap(),
                        body: self.into().into()
                    }
            }
        }


        impl ToString for #ident {
            fn to_string(&self) -> String {
                self.message.clone()
            }
        }

        #(
            impl From<#from> for #ident {
                fn from(e: #from ) -> Self {
                    Self {
                        message: e.to_string()
                    }
                }
            }
        )*
    };
    //println!("{}", rtn.to_string());
    rtn)
}

 */

#[proc_macro_derive(ToBase)]
pub fn base(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let ident = &input.ident;
    let base = format_ident!("{}Base", ident);
    let mut variants: Vec<Ident> = vec![];

    if let Data::Enum(data) = &input.data {
        for variant in data.variants.clone() {
            variants.push(variant.ident.clone());
        }
    }

    let rtn = quote! {
        pub enum #base {
        #(#variants),*
        }


        #[allow(bindings_with_variant_name)]
        impl ToString for #base {
            fn to_string(&self) -> String {
                match self {
            #( #variants => "#variants".to_string() ),*
                }
            }
        }
    };

    rtn.into()
}

#[proc_macro_derive(ToLogMark)]
pub fn to_log_mark(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let ident = &input.ident;
    let base = format_ident!("{}Base", ident);
    let mut variants: Vec<Ident> = vec![];

    if let Data::Enum(data) = &input.data {
        for variant in data.variants.clone() {
            variants.push(variant.ident.clone());
        }
    }

    let rtn = quote! {
        pub enum #base {
        #(#variants),*
        }


        #[allow(bindings_with_variant_name)]
        impl ToString for #base {
            fn to_string(&self) -> String {
                match self {
            #( #variants => "#variants".to_string() ),*
                }
            }
        }
    };

    rtn.into()
}

#[cfg(test)]
mod tests {}

/*
#[proc_macro_derive(EnumAsStr)]
pub fn directed_handler(item: TokenStream) -> TokenStream {
    TokenStream::from(quote! {})
}

 */

#[proc_macro_attribute]
pub fn loggerhead(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut out = vec![];
    let input = parse_macro_input!(item as File);
    for item in input.items.into_iter() {
        let item = quote!(#item);
        println!("running parser over {}", item);
        out.push(item);
    }

    let rtn = quote! {
        #(#out)*
    };

    rtn.into()
}

#[proc_macro]
pub fn push_loc(tokens: TokenStream) -> TokenStream {
    let crt = crt_name();
    let tuple = parse_macro_input!(tokens as ExprTuple);
    let mut iter = tuple.elems.into_iter();
    let logger = iter.next().unwrap();
    let loc = iter.next().unwrap();

    let rtn = quote! {
        {
    let mut builder = #crt::log::LogMarkBuilder::default();
    builder.package(env!("CARGO_PKG_NAME").to_string());
    builder.file(file!().to_string());
    builder.line(line!().to_string());
    let mark = builder.build().unwrap();
    #logger.push(#loc)
            }
        };

    rtn.into()
}

#[proc_macro]
pub fn log_span(tokens: TokenStream) -> TokenStream {
    let crt = crt_name();
    let input = parse_macro_input!(tokens as Expr);
    let rtn = quote! {
        {
    let mut builder = #crt::log::LogMarkBuilder::default();
    builder.package(env!("CARGO_PKG_NAME").to_string());
    builder.file(file!().to_string());
    builder.line(line!().to_string());
    let mark = builder.build().unwrap();
    #input.push_mark(mark)
            }
        };

    rtn.into()
}

#[proc_macro]
pub fn logger(item: TokenStream) -> TokenStream {
    let crt = crt_name();
    let log_pack = quote!(#crt::log);

    let loc = if !item.is_empty() {
        let expr = parse_macro_input!(item as Expr);
        quote!( #log_pack::logger().push(#expr); )
    } else {
        quote!( #log_pack::logger(); )
    };

    let rtn = quote! {
        {
            let logger = #loc;
    let mut builder = #log_pack::LogMarkBuilder::default();
    builder.package(env!("CARGO_PKG_NAME").to_string());
    builder.file(file!().to_string());
    builder.line(line!().to_string());
    let mark = builder.build().unwrap();
            logger.push_mark(mark)
            }
        };

    rtn.into()
}

#[proc_macro]
pub fn push_mark(_item: TokenStream) -> TokenStream {
    let crt = crt_name();
    let logger = parse_macro_input!(_item as Expr);
    let rtn = quote! {
        {
    let mut builder = #crt::log::LogMarkBuilder::default();
    builder.package(env!("CARGO_PKG_NAME").to_string());
    builder.file(file!().to_string());
    builder.line(line!().to_string());
    let mark  = builder.build().unwrap();
    #logger.push_mark(mark)
            }

        };

    rtn.into()
}

#[proc_macro]
pub fn create_mark(_item: TokenStream) -> TokenStream {
    let crt = crt_name();
    let rtn = quote! {
            {
    println!("CARGO_PKG_NAME: {}", env!("CARGO_PKG_NAME"));
        let mut builder = #crt::log::LogMarkBuilder::default();
        builder.package(env!("CARGO_PKG_NAME").to_string());
        builder.file(file!().to_string());
        builder.line(line!().to_string());
        builder.loc(Default::default());
        builder.build().unwrap()
                }
            };

    rtn.into()
}

#[proc_macro]
pub fn warn(_item: TokenStream) -> TokenStream {
    let crt = crt_name();
    let input = parse_macro_input!(_item as LitStr);
    let rtn = quote! {

    // pushing scope so we don't collide with
    // any other imports or local things...
    {
        use starlane_primitive_macros::mark;
        use starlane_primitive_macros::create_mark;
        use #crt::log::Log;
        use #crt::log::LOGGER;
        use #crt::log::root_logger;

        // need to push_mark somewhere around here...
        LOGGER.try_with(|logger| {
             logger.warn(stringify!(#input));
        } ).map_err(|e| {
        root_logger().warn(stringify!(#input));
    })
        }
     };
    rtn.into()
}

/*
#[proc_macro_attribute]
pub fn point_log(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut out = vec![];
    let input = parse_macro_input!(item as File);
    for item in input.items.into_iter() {
        let item = quote!(#item);
        println!("running parser over {}", item);
        out.push(item);
    }

    let rtn = quote! {
        #(#out)*
    };

    panic!("~~~ POINT LOG MACRO: {}",rtn.to_string());

    rtn.into()
}

 */

#[proc_macro_attribute]
pub fn log(attr: TokenStream, item: TokenStream) -> TokenStream {
    item.into()
}

#[proc_macro_attribute]
pub fn logger_att(attr: TokenStream, item: TokenStream) -> TokenStream {
    let surface = if attr.is_empty() {
        format_ident!("logger")
    } else {
        format_ident!("{}", attr.to_string())
    };

    let item_cp = item.clone();
    let mut impl_item = parse_macro_input!(item_cp as syn::ItemImpl);
    //    let mut wrappers = vec![];
    //    let mut methods = vec![];

    for item_impl in &impl_item.items {
        if let ImplItem::Method(call) = item_impl {
            {
                let (__async, __await) = match call.sig.asyncness {
                    None => (quote! {}, quote! {}),
                    Some(_) => (quote! {async}, quote! {.await}),
                };

                let mut inner_call = call.clone();
                inner_call.vis = Visibility::Inherited;
                inner_call.sig.ident = format_ident!("__{}", call.sig.ident);
                inner_call.attrs = vec![];
                /*
                let args: Vec<TokenStream>  = inner_call.sig.inputs.clone().into_iter().map( |arg| match arg.clone() {

                   FnArg::Receiver(r) => {
                      let arg = quote!{#r};
                       arg.to_token_stream()

                   },
                    arg => arg.to_token_stream()
                }

                ).collect_into();


                let args = quote!{#( #args )*};
                panic!("ARGS: {}",args.to_string());
                 */
                todo!();

                let attributes = call.attrs.clone();
                let vis = call.vis.clone();
                let sig = call.sig.clone();
                let block = call.block.clone();

                call.clone();
                let blah = quote! {
                   #(#attributes)*
                   #vis
                   #__async
                   #sig
                    {
                        #inner_call
                    }
                };
                panic!("{}", blah);
            }
        }
    }

    //    TokenStream2::from_iter(vec![rtn, TokenStream2::from(item)]).into()
    todo!()
}

fn find_impl_type(item_impl: &ItemImpl) -> Ident {
    if let Type::Path(path) = &*item_impl.self_ty {
        path.path.segments.last().as_ref().unwrap().ident.clone()
    } else {
        panic!("could not get impl name")
    }
}

fn find_log_attr(attrs: &Vec<Attribute>) -> TokenStream {
    for attr in attrs {
        if attr
            .path
            .segments
            .last()
            .expect("segment")
            .to_token_stream()
            .to_string()
            .as_str()
            == "logger"
        {
            let rtn = quote!(#attr);
            return rtn.into();
        }
    }
    let rtn = quote!(logger);
    rtn.into()
}

