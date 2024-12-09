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
use convert_case::{Case, Casing};
use deluxe::HasAttributes;
use quote::{format_ident, quote, ToTokens};
use syn::{parse2, parse_macro_input, parse_quote, Attribute, FnArg, ItemMod, Meta, MetaNameValue, PatType, ReturnType, Signature, Token, TraitItem, Type};
use syn::parse::ParseStream;
use syn::punctuated::Punctuated;
use syn::token::{At, In};



/*
fn proxy_attr(item: proc_macro2::TokenStream) -> deluxe::Result<attributes> {
    let mut input = syn::parse2::<syn::DeriveInput>(item)?;

    // Extract the attributes!
    let attributes: ProxyAttributes = deluxe::extract_attributes(&mut input)?;

    // Now get some info to generate an associated function...
    let ident = &input.ident;
    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();

    Ok(quote::quote! {
        impl #impl_generics #ident #type_generics #where_clause {
            fn my_desc() -> &'static str {
                concat!("Name: ", #name, ", Version: ", #version)
            }
        }
    })
}

 */

pub(crate) fn proxy(attrs: TokenStream, proxy_trait: TokenStream,) -> TokenStream {

    let res : syn::Result<Meta> = syn::parse(attrs);
    let prefix =
    match res {
        Ok(Meta::NameValue(MetaNameValue{
            path,
            value,
            ..
        })) if path.to_token_stream().to_string().as_str() == "prefix " => {
            Some(value.to_token_stream().to_string())
        },
        _ =>  None
    };


    let mut builder = ProxyFactoryBuilder::default();
    let mut methods = vec![];


    if let Some(prefix) = prefix {
        builder.prefix(prefix);
    }

    let proxy_trait_cp = proxy_trait.clone().into();
    let r#trait = parse_macro_input!(proxy_trait_cp as syn::ItemTrait);
    builder.ident(r#trait.ident.into());

    for item in &r#trait.items {
        if let TraitItem::Fn(method) = item {
            let mut fac= MethodFactoryBuilder::default();
            fac.sig(method.sig.clone());
            methods.push(fac.build().unwrap());
        }
    }

    let has_async = methods.iter().any(|method|method.is_async());
    builder.has_async(has_async);
    builder.methods(methods);
    let factory =  builder.build().unwrap();



    let decl = factory.call();
    let tx = factory.tx();


    let expanded = quote!{   #proxy_trait };

    expanded.into()
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
    pub fn ty(&self) -> &TokenStream {
        &self.ty
    }
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

struct Names {
    trait_name: Ident,
    tx: Ident,
    call: Ident,
    runner: Ident,
}

#[derive(Clone,derive_builder::Builder)]
pub struct ProxyFactory {

    #[builder(setter(into, strip_option), default)]
    pub prefix: Option<String>,
    pub ident: Ident,
    #[builder(setter(into, strip_option), default)]
    pub methods: Vec<MethodFactory>,
    pub has_async: bool,
}

impl ProxyFactory {

    fn names(&self) -> Names {

        let common = match &self.prefix {
            None =>  self.ident.clone(),
            Some(prefix) => format_ident!("{}{}", prefix,self.ident),
        };

        let trait_name = self.ident.clone();
        let tx =  format_ident!("{}Tx", common);
        let call =  format_ident!("{}Call", common);
        let runner =  format_ident!("{}Runner", common);

        Names {
            trait_name,
            tx,
            call,
            runner,
        }
    }

    /// the proxy transmitter
    fn tx(&self) -> TokenStream {

        let Names{ trait_name, tx, call, runner } = self.names();
        let tx_methods =  self.methods.iter().map( |method| method.tx(&self.names()).collect::<Vec<_>>();

        let expand= quote!{
            pub struct #tx {
                call_tx: tokio::sync::oneshot::Sender<#call>,
            }

            impl #tx {
                pub fn new(call_tx: tokio::sync::oneshot::Sender<#call>) -> Self {
                   Self{ call_tx }
                }
            }

            impl #trait_name for #tx {
                #( #tx_methods )*
            }
        };

        expand.into()
    }

    fn runner_handler(&self) -> TokenStream {

        let Names{ trait_name, tx, call, runner } = self.names();
        let tx_methods =  self.methods.iter().map( |method| method.tx(&self.names())).collect::<Vec<_>>();

        let expand= quote!{
            pub struct #tx {
                call_tx: tokio::sync::oneshot::Sender<#call>,
            }

            impl #tx {
                pub fn new(call_tx: tokio::sync::oneshot::Sender<#call>) -> Self {
                   Self{ call_tx }
                }
            }

            impl #trait_name for #tx {
                #( #tx_methods )*
            }
        };

        expand.into()
    }

    /// implement the `Call` enum
    fn call(&self) -> TokenStream {
        let Names{ trait_name, tx, call, runner } = self.names();
        let variants: Vec<TokenStream> = self.methods.iter().map(MethodFactory::call).collect();
        let ident = &self.ident.to_string();
        let decl =quote!{
               enum  #ident {
                   #(#call::#variants),*
               }
            };

        decl.to_token_stream().into()
    }


}


impl From<TokenStream> for MyArg {
    fn from(ty: TokenStream) -> Self {
        let ident = format_ident!("rtn").into();
        MyArg { ident, ty }
    }
}

impl MethodFactory {

    fn variant_name(&self) -> String {
        self.sig.ident.to_string().from_case(Case::Snake).to_case(Case::UpperSnake)
    }

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

    fn tx(&self, names: &Names) ->  TokenStream {
        let Names{ trait_name, tx, call, runner } = names;

        let method= &self.sig.ident;

        //        let types = args.iter().map(MyArg::ty).collect::<Vec<_>>();
        /// first figure what args send
        let args = self.args_with_rtn();
        let constructor= if args.is_empty() {
            quote!{ #call::#method }
        } else if args.len() == 1 {
            /// we can safely [Options::unwrap] because we just confirmed 1 args
            let arg_name= args.first().map(|name| &name.ident).unwrap();
            quote!{ #call::#method(#arg_name) }
        } else {
            let names = args.iter().map(|name| &name.ident).collect::<Vec<_>>();
            quote!{ #call::#method{ #(#names),* }}
        };

        let (send,recv) = if self.is_async() {
            (quote!{call_tx.send(#constructor).await?;}, quote!{call_rx.recv().await?})
        } else {
            (quote!{call_tx.try_send(#constructor)?;}, quote!{call_rx.blocking_recv()?})
        };

        let sig = &self.sig;
        let payload = if self.return_type().is_some() {
            quote!{
                  #sig  {
                    let (rtn_tx,mut rtn_rx) = #ONESHOT::channel();
                    #send
                    #recv
                  }
                }
        } else {
            quote!{
                    #sig {
                    #send
                    }
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
    fn call(&self) -> TokenStream {
        let variant = self.variant_name();
        let args =  self.args_with_rtn();
        let tokens: proc_macro2::TokenStream= if args.is_empty() {
            quote!{ #variant }
        } else if args.len() == 1 {
            let ty = &args.first().unwrap().ty.to_string();
            quote!{ #variant(#ty) }
        } else {
            quote!{ #variant{ #( #args ),* } }
        };
        tokens.into()
    }


    fn runner_handler(&self, names: &Names) ->  TokenStream {
        let Names{ trait_name, tx, call, runner } = names;
        let variant = self.variant_name();
        let args = self.args_with_rtn();
        if args.is_empty() {
            quote!{
                #call::#variant  => {

                }
            }
        }


        let expand = quote! {

            #call::#variant =>

        };

        expand.into()
    }

}