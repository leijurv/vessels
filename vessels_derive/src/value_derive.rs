use crate::prefix;
use syn::Ident;
use proc_macro2::Span;

pub(crate) fn value_derive(mut s: synstructure::Structure) -> proc_macro2::TokenStream {
    let ast = s.ast();
    let ident = &ast.ident;
    if s.variants().is_empty() {
        return quote_spanned! {
            ident.span() =>
            compile_error!("Value cannot be derived for an enum with no variants");
        };
    }
    let en = prefix(ident, "Derive_Variants");
    let mut stream = proc_macro2::TokenStream::new();
    let mut variants = proc_macro2::TokenStream::new();
    let mut serialize_impl = proc_macro2::TokenStream::new();
    let mut deserialize_impl = proc_macro2::TokenStream::new();
    let mut id: usize = 0;
    s.variants().iter().for_each(|variant| {
        let ident = &variant.ast().ident;
        let base = format!("{}_AssertValue_", ident);
        let bindings = variant.bindings();
        bindings.iter().enumerate().for_each(|(index, binding)| {
            let name = prefix(&ast.ident, &(base.clone() + &index.to_string()));
            let ident = Ident::new(&format!("{}_{}", ident, index), Span::call_site());
            let ty = &binding.ast().ty;
            variants.extend(quote! {
                #ident(<#ty as ::vessels::protocol::Value>::Item),
            });
            stream.extend(quote! {
                struct #name where #ty: ::vessels::protocol::Value;
            });
            serialize_impl.extend(quote! {
                #en::#ident(data) => {
                    let mut seq = serializer.serialize_seq(Some(2))?;
                    seq.serialize_element(&#id)?;
                    seq.serialize_element(data)?;
                    seq.end()
                }
            });
            deserialize_impl.extend(quote! {
                #id => {
                    #en::#ident(seq.next_element()?.ok_or_else(|| ::serde::de::Error::invalid_length(1, &self))?)
                }
            });
            id += 1;
        });
        if bindings.is_empty() {
            variants.extend(quote! {
                #ident,
            });
            serialize_impl.extend(quote! {
                #en::#ident => {
                    let mut seq = serializer.serialize_seq(Some(1))?;
                    seq.serialize_element(&#id)?;
                    seq.end()
                }
            });
            deserialize_impl.extend(quote! {
                #id => {
                    #en::#ident
                }
            });
        }
    });
    let expectation = format!("a serialized Value item from the derivation on {}", ident);
    stream.extend(quote! {
        #[doc(hidden)]
        pub enum #en {
            #variants
        }
        impl ::serde::Serialize for #en {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: ::serde::Serializer {
                use ::serde::ser::SerializeSeq;
                match self {
                    #serialize_impl
                }
            }
        }
        impl<'de> ::serde::Deserialize<'de> for #en {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: ::serde::Deserializer<'de> {
                struct CallVisitor;
                impl<'de> ::serde::de::Visitor<'de> for CallVisitor {
                    type Value = #en;

                    fn expecting(&self, formatter: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                        formatter.write_str(#expectation)
                    }
                    fn visit_seq<V>(self, mut seq: V) -> Result<#en, V::Error> where V: ::serde::de::SeqAccess<'de>, {
                        let index: usize = seq.next_element()?.ok_or_else(|| ::serde::de::Error::invalid_length(0, &self))?;
                        Ok(match index {
                            #deserialize_impl
                            _ => { Err(::serde::de::Error::invalid_length(0, &self))? }
                        })
                    }
                }
                deserializer.deserialize_seq(CallVisitor)
            }
        }
    });
    s.bind_with(|_| synstructure::BindStyle::Move);
    let mut return_stream = proc_macro2::TokenStream::new();
    let mut decl_stream = proc_macro2::TokenStream::new();
    let mut select_stream = proc_macro2::TokenStream::new();
    let mut idx = 0;
    let deconstruct = s.each_variant(|variant| {
        let ident = &variant.ast().ident;
        let bindings = variant.bindings();
        if bindings.is_empty() {
            return quote! {
                sink.start_send(#en::#ident).unwrap();
            };
        };
        let mut stream = proc_macro2::TokenStream::new();
        bindings.iter().enumerate().for_each(|(index, bi)| {
            let pat = &bi.pat();
            let ty = &bi.ast().ty;
            let r_ident = Ident::new(&format!("{}_{}", ident, index), Span::call_site());
            let ident = Ident::new(&format!("{}_{}_ct", ident, index), Span::call_site());
            let ident_ctx = Ident::new(&format!("{}_{}_ctx", ident, index), Span::call_site());
            stream.extend(quote! {
                let ctxs = ::vessels::protocol::Context::new();
                let (i_sink, i_stream) = ctxs.1.split();
                #ident_ctx = Some(i_sink);
                #ident = Some(i_stream);
                #pat.deconstruct(ctxs.0);
            });
            return_stream.extend(quote! {
                #en::#r_ident(data) => {
                    let mut s = None;
                    ::std::mem::swap(&mut s, &mut #ident_ctx);
                    ::vessels::executor::spawn(s.expect("No split sink").send_all(::futures::stream::once(Ok(data)).chain(item.1.filter_map(|item| {
                        if let #en::#r_ident(item) = item {
                            Some(item)
                        } else {
                            None
                        }
                    }))).map_err(|e| {
                        println!("{:?}", e);
                        e
                    }).then(|_| Ok(())));
                }
            });
            select_stream.extend(quote! {
                let sel_stream = (if let Some(stream) = #ident { Box::new(stream.map(|item| #en::#r_ident(item)).select(sel_stream)) } else { sel_stream });
            });
            decl_stream.extend(quote! {
                let (mut #ident, mut #ident_ctx): (Option<::futures::stream::SplitStream<::vessels::protocol::Context::<<#ty as ::vessels::protocol::Value>::Item>>>, Option<::futures::stream::SplitSink<::vessels::protocol::Context::<<#ty as ::vessels::protocol::Value>::Item>>>) = (None, None);
            });
            idx += 1;
        });
        stream
    });
    let mut construct = proc_macro2::TokenStream::new();
    s.variants().iter().for_each(|variant| {
        let v_ident = variant.ast().ident;
        let pat = &variant.pat();
        let bindings = variant.bindings();
        if bindings.is_empty() {
            construct.extend(quote! {
                #en::#v_ident => { Ok(#pat) }
            });
            return;
        }
        (0..bindings.len()).for_each(|index| {
            let mut decl_stream = proc_macro2::TokenStream::new();
            let mut select_stream = proc_macro2::TokenStream::new();
            let mut item_stream = proc_macro2::TokenStream::new();
            let b_ident = Ident::new(&format!("{}_{}", v_ident, index), Span::call_site());
            let cst = variant.construct(|field, idx| {
                let b_i_ident = Ident::new(&format!("{}_{}", v_ident, idx), Span::call_site());
                let ident_ct = Ident::new(&format!("{}_{}_ct", v_ident, idx), Span::call_site());
                let ident_ctx = Ident::new(&format!("{}_{}_ctx", ident_ct, idx), Span::call_site());
                let ident_ctxs = Ident::new(&format!("{}_{}_ctxs", ident_ct, idx), Span::call_site());
                let ty = &field.ty;
                decl_stream.extend(quote! {
                    let (mut #ident_ct, mut #ident_ctx): (::futures::stream::SplitStream<::vessels::protocol::Context::<<#ty as ::vessels::protocol::Value>::Item>>, ::futures::stream::SplitSink<::vessels::protocol::Context::<<#ty as ::vessels::protocol::Value>::Item>>);
                });
                select_stream.extend(quote! {
                    let sel_stream = #ident_ct.map(|item| #en::#b_i_ident(item)).select(sel_stream);
                });
                item_stream.extend(quote! {
                    #en::#b_i_ident(item) => {
                        #ident_ctx.start_send(item).unwrap();
                    }
                });
                quote! {
                    {
                        let ret = <#ty as ::vessels::protocol::Value>::construct(#ident_ctxs);
                        ret
                    }
                }
            });
            let mut mcst = proc_macro2::TokenStream::new();
            variant.bindings().iter().enumerate().for_each(|(idx, field)| {
                let ident_ct = Ident::new(&format!("{}_{}_ct", v_ident, idx), Span::call_site());
                let ident_ctx = Ident::new(&format!("{}_{}_ctx", ident_ct, idx), Span::call_site());
                let ident_ctxs = Ident::new(&format!("{}_{}_ctxs", ident_ct, idx), Span::call_site());
                let ty = &field.ast().ty;
                decl_stream.extend(quote! {
                    let #ident_ctxs: ::vessels::protocol::Context<<#ty as ::vessels::protocol::Value>::Item>;
                });
                mcst.extend(quote! {
                    {
                        let ctxs = ::vessels::protocol::Context::new();
                        let (i_sink, i_stream) = ctxs.1.split();
                        #ident_ctx = i_sink;
                        #ident_ct = i_stream;
                        #ident_ctxs = ctxs.0;
                    }
                });
            });
            construct.extend(quote! {
                #en::#b_ident(data) => {
                    let sel_stream = ::futures::stream::empty();
                    #decl_stream
                    #mcst;
                    ::vessels::executor::spawn(::futures::stream::once(Ok(#en::#b_ident(data))).chain(v.1).for_each(move |item| {
                        match item {
                            #item_stream
                            _ => {}
                        };
                        Ok(())
                    }));
                    #select_stream
                    ::vessels::executor::spawn(sel_stream.forward(sink).map_err(|e| {
                        println!("{:?}", e);
                        e
                    }).then(|_| Ok(())));
                    let ret = Ok(#cst);
                    ret
                }
            });
        });
    });
    let wrapper_ident = prefix(ident, "Derive_Container");
    stream.extend(quote! {
        impl ::vessels::protocol::Value for #ident {
            type Item = #en;

            fn deconstruct<
                C: ::futures::Sink<SinkItem = Self::Item, SinkError = ()>
                    + ::futures::Stream<Item = Self::Item, Error = ()>
                    + Send
                    + 'static,
            >(
                self,
                context: C,
            ) where
                Self: Sized,
            {
                use ::futures::{Sink, Stream};
                let (mut sink, mut stream) = context.split();
                let sel_stream: Box<dyn Stream<Item = Self::Item, Error = ()> + Send> = Box::new(::futures::stream::empty());
                #decl_stream
                match self {
                    #deconstruct
                };
                ::vessels::executor::spawn(stream.into_future().map_err(|e| {
                        println!("{:?}", e.0);
                        ()
                    }).and_then(move |item| {
                        let i = item.0.unwrap();
                        match i {
                            #return_stream
                            _ => {}
                        };
                        Ok(())
                    }
                ));
                #select_stream
                ::vessels::executor::spawn(sel_stream.forward(sink).map_err(|e| {
                        println!("{:?}", e);
                        e
                    }).then(|_| Ok(())));
            }
            fn construct<
                C: ::futures::Sink<SinkItem = Self::Item, SinkError = ()>
                    + ::futures::Stream<Item = Self::Item, Error = ()>
                    + Send
                    + 'static,
            >(
                context: C,
            ) -> Self {
                use ::futures::{Sink, Stream};
                let (sink, stream) = context.split();
                if let Ok(constructed) = stream.into_future().and_then(|v| {
                    match v.0.unwrap() {
                        #construct
                    }
                }).wait() {
                    constructed
                } else {
                    panic!("Invalid return in derived Value construction")
                }
                
            }
        }
    });
    quote! {
        const #wrapper_ident: () = {
            #stream
        };
    }
}