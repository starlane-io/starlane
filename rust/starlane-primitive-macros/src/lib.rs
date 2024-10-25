/*#![feature(proc_macro_quote)]*/
#![crate_type = "lib"]
#![allow(warnings)]
#[feature("proc_macro_lib2")]

#[macro_use]
extern crate quote;

use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use quote::ToTokens;
use syn::{parse_file, parse_macro_input, Data, DeriveInput, File, PathArguments, Token, Type};
use syn::__private::TokenStream2;

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
        quote!(starlane_space::err::ParseErrs),
        quote!(String),
        quote!(&'static str),
        quote!(mechtron::err::GuestErr),
        quote!(std::string::FromUtf8Error),
    ];

    let rtn = quote! {

        impl MechErr for #ident {
            fn to_uni_err(self) -> starlane_space::err::{
               starlane_space::err::SpaceErr::server_error(self.to_string())
            }
        }

        impl From<#ident> for mechtron::err::GuestErr{
            fn from(e: #ident) -> Self {
                        mechtron::err::GuestErr {
                            message: e.to_string()
                        }
            }
        }

        impl starlane_space::err::CoreReflector for #ident {
                fn as_reflected_core(self) -> starlane_space::wave::core::ReflectedCore {
                   starlane_space::wave::core::ReflectedCore{
                        headers: Default::default(),
                        status: starlane_space::wave::core::http2::StatusCode::from_u16(500u16).unwrap(),
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
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}


#[proc_macro_derive(EnumAsStr)]
pub fn directed_handler(item: TokenStream) -> TokenStream {
    TokenStream::from(quote! {})
}




#[proc_macro_attribute]
pub fn loggerize(_attr: TokenStream, item: TokenStream) -> TokenStream {


    let mut out = vec![];
    let input = parse_macro_input!(item as File);
    for item in input.items.into_iter() {
        let item = quote!(#item);
println!("running parser over {}",item);
        out.push(item);
     }


    let rtn = quote! {
        #(#out)*
    };

    rtn.into()
}


#[proc_macro]
pub fn mark(_item: TokenStream) -> TokenStream {
    let rtn = quote! {
    let mut builder = LogMarkBuilder::default();
    builder.package(env!("CARGO_PKG_NAME").to_string());
    builder.file(file!().to_string());
    builder.line(line!().to_string());
    builder.build().unwrap();
        };

   rtn.into()
}


#[proc_macro]
pub fn create_mark(_item: TokenStream) -> TokenStream {
    let rtn = quote! {
    let mut builder = LogMarkBuilder::default();
    builder.package(env!("CARGO_PKG_NAME").to_string());
    builder.file(file!().to_string());
    builder.line(line!().to_string());
    builder.build().unwrap();
        };

    rtn.into()
}



