/*#![feature(proc_macro_quote)]*/
#![crate_type = "lib"]
#![allow(warnings)]

#[macro_use]
extern crate quote;

use proc_macro::TokenStream;
use quote::quote;
use quote::ToTokens;
use syn::__private::TokenStream2;
use syn::{parse_macro_input, Data, DeriveInput, Type, PathArguments};
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
                                PathArguments::AngleBracketed(ty) => format_ident!("{}",ty.args.to_token_stream().to_string() ),
                                _ => panic!("expecting angle brackets")
                            };

                            let ty_str = ty.to_string();

                            xforms.push(quote! {
                        impl TryInto<#ty> for #ident {
                            type Error=MsgErr;

                            fn try_into(self) -> Result<#ty,Self::Error> {
                                match self {
                                Self::#variant_ident(val) => Ok(*val),
                                _ => Err(format!("expected {}",#ty_str).into())
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
                                    type Error=MsgErr;

                                    fn try_into(self) -> Result<#ty,Self::Error> {
                                        match self {
                                            Self::#variant_ident(val) => Ok(val),
                                            _ => Err(format!("expected {}",#ty_str).into())
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

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
