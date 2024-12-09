/*#![doc(html_root_url = "https://docs.rs/async-trait/0.1.83")]
#![allow(
clippy::default_trait_access,
clippy::doc_markdown,
clippy::explicit_auto_deref,
clippy::if_not_else,
clippy::items_after_statements,
clippy::match_like_matches_macro,
clippy::module_name_repetitions,
clippy::shadow_unrelated,
clippy::similar_names,
clippy::too_many_lines,
clippy::trivially_copy_pass_by_ref
)]

 */

static ONESHOT: &str = "tokio::sync::oneshot";

use proc_macro2::Ident;
use proc_macro::TokenStream;
use std::borrow::Borrow;
use quote::{format_ident, quote, ToTokens};
use syn::{parse_macro_input, Attribute, FnArg, PatType, ReturnType, Signature, Token, TraitItem, Type};
use syn::punctuated::Punctuated;

pub(crate) fn proxy(attrs: TokenStream, proxy_trait: TokenStream,) -> TokenStream {

    let mut builder = ProxyFactoryBuilder::default();
    let proxy_trait_cp = proxy_trait.clone().into();
    let r#trait = parse_macro_input!(proxy_trait_cp as syn::ItemTrait);
    builder.ident(r#trait.ident.into());


    /*
    let attrs = attrs.into();
    let attrs = parse_macro_input!(attrs as Attribute::parse_outer);
    for attr in attrs {
        match &attr {
            NestedMeta::Meta(Meta::NameValue(MetaNameValue{ path, lit , .. })) if path.is_ident("prefix") =>  {
                builder.prefix(lit.to_token_stream().to_string().replace("\"",""));
            }
            x => panic!("attribute not expected: {}",x.to_token_stream())
        }
    }

     */

    for item in &r#trait.items {
        if let TraitItem::Fn(method) = item {
            let mut fac= MethodFactoryBuilder::default();
            fac.sig(method.sig.clone());
        }
    }


    panic!();
}




fn is_no_proxy(attr: &Vec<Attribute>) -> bool {
    crate::find_attr("no_proxy", attr).is_some()
}


#[derive(derive_builder::Builder,Clone)]
struct MethodFactory {
    pub sig: Signature,
}

impl MethodFactory {

    pub fn ident(&self) -> Ident {
        self.sig.ident.clone().into()
    }

    pub fn args( &self ) -> Vec<MyArg> {
        MyArg::from(&self.sig.inputs)
    }

    pub fn has_return_type(&self) -> bool {
        self.return_type().is_some()
    }

    pub fn return_type(&self) -> Option<Type> {
        match &self.sig.output {
            ReturnType::Default => None,
            ReturnType::Type(_, rtn) => { Some(Clone::clone(rtn)) }
        }
    }

}





impl MethodFactory {
    fn is_async(&self) -> bool {
        self.sig.asyncness.is_some()
    }
}

struct MyArg {
    pub ident: Ident,
    pub ty: TokenStream,
}

impl MyArg {


    fn from(args:&Punctuated<FnArg, Token![,]>) -> Vec<MyArg> {

        fn is_receiver(arg: &&FnArg) -> bool {
            let arg = *arg;
            to_pat(arg).is_some()
        }


        fn to_pat( arg: &FnArg) -> Option<&PatType> {
            match arg {
                FnArg::Receiver(_) => None,
                FnArg::Typed(pat) => Some(pat)
            }
        }

        fn from_pat( pat: &PatType ) -> MyArg {
            let ident = format_ident!("{}",pat.pat.to_token_stream().to_string()).into();
            let ty = pat.to_token_stream().into();

            MyArg {
                ident,
                ty
            }
        }


        args.iter().filter(is_receiver).map(to_pat).map(Option::unwrap).map(from_pat).collect::<Vec<_>>()
    }
}

impl ToTokens for MyArg {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let ident = &self.ident;
        let ty = &self.ty;
        let ident : proc_macro2::TokenStream = quote!{#ident};
        let ty: proc_macro2::TokenStream = format_ident!("{}",ty.to_string()).to_token_stream();
        let ident : proc_macro2::TokenStream = format_ident!("{}",ident.to_string()).to_token_stream();


        let arg: proc_macro2::TokenStream = quote!{#ident: #ty};
        arg.to_tokens(tokens);
    }
}

impl Into<Ident> for MyArg {
    fn into(self) -> Ident {
        self.ident
    }
}





///let filter : Fn(&Vec<dyn FnArgs>) -> Vec<Arg>  =  |a: &args:Vec<FnArg>| {


#[derive(Clone,derive_builder::Builder)]
pub struct ProxyFactory {
    pub prefix: String,
    pub ident: Ident,
    pub methods: Vec<MethodFactory>,
    pub has_async: bool,
}

impl ProxyFactory {

    fn decl(&self) -> proc_macro2::TokenStream {
        let methods: Vec<String> = self.methods.iter().map(MethodFactory::decl).map(|m| m.to_string()).collect();
        let ident = &self.ident.to_string();

        let decl =quote!{
               enum  #ident {
                   #(#methods),*
               }
            };

        let decl = "enum Mode { A, B }";

        //let decl = syn::parse_str(decl.to_string().as_str()).unwrap();
        let decl = syn::parse_str(decl).unwrap();
        decl
    }
}


impl From<TokenStream> for MyArg {
    fn from(ty: TokenStream) -> Self {
        let ident = format_ident!("rtn").into();
        MyArg { ident, ty }
    }
}

impl MethodFactory {

    fn rtn_as_arg(&self) -> Option<MyArg> {
        match &self.return_type(){
            Some(ty) => {
                Some(MyArg {
                    ident: format_ident!("rtn").into(),
                    ty: quote!{ #ONESHOT::Sender<#ty>}.into()
                })
            },
            None => None
        }
    }


    fn args_with_rtn(&self) -> Vec<MyArg> {
        let mut args =self.args();
        if self.has_return_type() {
            args.push(self.rtn_as_arg().unwrap());
        }
        args
    }

    fn send(&self, factory: &ProxyFactory) ->  TokenStream {
        let proxy = &factory.ident;
        let method= &self.sig.ident;

        /// first figure what args send
        let args = self.args_with_rtn();

        let new = if args.is_empty() {
            quote!{ #proxy::#method }
        } else if args.len() == 1 {
            let ty: Type = syn::parse2(args.first().unwrap().clone().to_token_stream()).unwrap();
            quote!{ #proxy::#method(#ty) }
        } else {
            quote!{ #proxy::#method{ #(#args),* }}
        };

        let (send,recv) = if self.is_async() {
            (quote!{call_tx.send(#new).await?;},quote!{call_rx.recv().await?})
        } else {
            (quote!{call_tx.try_send(#new)?;},quote!{call_rx.blocking_recv()?})
        };

        let payload = if self.return_type().is_some() {
            quote!{
                    let (rtn_tx,mut rtn_rx) = #ONESHOT::channel();
                    #send
                    #recv
                }
        } else {
            quote!{
                    #send
                }
        };

        payload.into()
    }


    /// this is where we `call` enum:
    /// ```
    /// enum MyCal {
    ///   SayHello,
    ///   HowAreYou(tokio::sync::oneshot::Sender<Result<String,Error>)
    ///   ManBitesDog{ man: String, dog: String, rtn: tokio::sync::oneshot::Sender<Result<String,Error> }
    /// }
    /// ```
    fn decl(&self) -> TokenStream {
        let method = &self.sig.ident;
        let args =  self.args_with_rtn();
        let tokens: proc_macro2::TokenStream= if args.is_empty() {
            quote!{ #method }
        } else if args.len() == 1 {
            let ty = &args.first().unwrap().ty.to_string();
            quote!{ #method(#ty) }
        } else {
            quote!{ #method{ #( #args ),* } }
        };
        tokens.into()
    }

}