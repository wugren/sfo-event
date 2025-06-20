use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use syn::{parse_macro_input, ConstParam, FnArg, GenericParam, Generics, ItemTrait, ReturnType, Token, TraitItem, TypeParam};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::token::Async;

#[proc_macro_attribute]
pub fn sfo_event(_args: proc_macro::TokenStream, input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let item = parse_macro_input!(input as ItemTrait);
    impl_macro(item).unwrap_or_else(to_compile_errors).into()
}

fn to_compile_errors(errors: Vec<syn::Error>) -> TokenStream {
    let compile_errors = errors.iter().map(syn::Error::to_compile_error);
    quote!(#(#compile_errors)*)
}

fn impl_macro(input: ItemTrait) -> Result<TokenStream, Vec<syn::Error>> {
    let emitter = generate_emitter_def(&input)?;
    let expanded = quote! {
        #input

        #emitter
    };
    Ok(expanded)
}

fn generate_emitter_def(input: &ItemTrait) -> Result<TokenStream, Vec<syn::Error>>{
    let generics = &input.generics;
    let name = &input.ident;
    let where_clause = &input.generics.where_clause;
    let emitter_name = Ident::new(&format!("{}Emitter", name), Span::call_site());

    let generic_input = generate_generic_input(generics);
    let emiter_impl = generate_emitter_impl(&input.items);
    Ok(quote! {
        pub struct #emitter_name #generics #where_clause {
            id: std::sync::atomic::AtomicU64,
            listeners: std::sync::Mutex<Vec<(u64, std::sync::Arc<dyn #name #generic_input>)>>,
        }

        impl #generics #emitter_name #generic_input #where_clause {
            pub fn new() -> Self {
                Self {
                    id: std::sync::atomic::AtomicU64::new(0),
                    listeners: std::sync::Mutex::new(Vec::new())
                }
            }

            pub fn clear(&self) {
                let mut _listeners = self.listeners.lock().unwrap();
                _listeners.clear()
            }

            pub fn add_listener(&self, listener: impl #name #generic_input) -> u64 {
                let id = self.id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let mut _listeners = self.listeners.lock().unwrap();
                _listeners.push((id, std::sync::Arc::new(listener)));
                id
            }

            pub fn remove_listener(&self, id: u64) {
                let mut _listeners = self.listeners.lock().unwrap();
                _listeners.retain(|(id_, _)| id_ != &id);
            }
            #(#emiter_impl)*
        }
    })
}

fn generate_emitter_impl(trait_items: &Vec<TraitItem>) -> Vec<TokenStream> {
    let mut impl_items = Vec::new();
    for trait_item in trait_items.iter() {
        if let TraitItem::Fn(method) = trait_item {
            let mut emitter_fn = method.clone();
            emitter_fn.sig.output = ReturnType::Default;
            emitter_fn.semi_token = None;
            let method_ident = &method.sig.ident;
            let method_output = &method.sig.output;
            // 检查返回类型是否是async_trait生成的Future
            let is_async_trait = match method_output {
                ReturnType::Type(_, ty) => {
                    let ty_str = quote!(#ty).to_string();
                    // println!("{}", ty_str);
                    ty_str.contains("core :: future :: Future") || ty_str.contains("std :: future :: Future")
                }
                ReturnType::Default => false
            };

            if is_async_trait {
                emitter_fn.sig.asyncness = Some(Async::default());
            }
            let sig = &emitter_fn.sig;
            let input_params = method.sig.inputs.iter().filter(|v| {
                match v {
                    FnArg::Receiver(_) => {
                        false
                    }
                    FnArg::Typed(_) => {
                        true
                    }
                }
            }).map(|v| {
                match v {
                    FnArg::Receiver(_) => {
                        unreachable!()
                    }
                    FnArg::Typed(ty) => {
                        let pat = &ty.pat;
                        quote! {
                            #pat
                        }
                    }
                }
            }).collect::<Punctuated<TokenStream, Token![,]>>();
            let item = if method.sig.asyncness.is_some() || is_async_trait {
                quote! {
                    pub #sig {
                        let _listeners = {
                            let _listeners = self.listeners.lock().unwrap();
                            let mut list = Vec::new();
                            for (_, listener) in _listeners.iter() {
                                list.push(listener.clone());
                            }
                            list
                        };
                        for listener in _listeners.iter() {
                            let _ = listener.#method_ident(#input_params).await;
                        }
                    }
                }
            } else {
                quote! {
                    pub #sig {
                        let _listeners = {
                            let _listeners = self.listeners.lock().unwrap();
                            let mut list = Vec::new();
                            for (_, listener) in _listeners.iter() {
                                list.push(listener.clone());
                            }
                            list
                        };
                        for listener in _listeners.iter() {
                            let _ = listener.#method_ident(#input_params);
                        }
                    }
                }
            };

            impl_items.push(item);
        }
    }
    impl_items
}
fn generate_generic_input(generics: &Generics) -> Generics {
    let gparams = generics.params.iter().map(|v| match v {
        GenericParam::Lifetime(_) => {
            v.clone()
        }
        GenericParam::Type(ty) => {
            GenericParam::Type(TypeParam {
                attrs: vec![],
                ident: ty.ident.clone(),
                colon_token: None,
                bounds: Default::default(),
                eq_token: None,
                default: None,
            })
        }
        GenericParam::Const(co) => {
            GenericParam::Const(ConstParam {
                attrs: vec![],
                const_token: Default::default(),
                ident: co.ident.clone(),
                colon_token: Default::default(),
                ty: co.ty.clone(),
                eq_token: None,
                default: None,
            })
        }
    }).collect::<Punctuated<GenericParam, Token![,]>>();
    let mut gen_generics = generics.clone();
    gen_generics.params = gparams;
    gen_generics
}
