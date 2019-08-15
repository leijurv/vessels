use crate::proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use syn::{parse_macro_input, spanned::Spanned, ItemImpl};

pub(crate) fn export(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        return r#"compile_error!("unexpected arguments passed to `export`");"#
            .parse()
            .unwrap();
    }
    let input = {
        let item = item.clone();
        parse_macro_input!(item as ItemImpl)
    };
    let proto: proc_macro2::TokenStream;
    let proto_t: proc_macro2::TokenStream;
    if let Some((_, path, _)) = input.trait_ {
        proto_t = quote! { #path };
        proto = quote! { dyn #proto_t };
    } else {
        return TokenStream::from(quote_spanned! {
            input.impl_token.span() =>
            compile_error!("`export` must be used on a trait implementation");
        });
    }
    let self_ty = *input.self_ty;
    let mut pfix = quote! {
        struct EXPORT_ASSERT_HAS_DEFAULT where #self_ty: ::std::default::Default;
        struct EXPORT_ASSERT_IS_PROTOCOL where #proto: ::vessels::protocol::Protocol;
        trait EXPORT_CONCRETE_BOUND: Send + ::vessels::macro_deps::futures::Sink<SinkItem = <#proto_t as ::vessels::protocol::Protocol>::Call, SinkError = ()> {}
        impl<T> EXPORT_CONCRETE_BOUND for T where T: Send + ::vessels::macro_deps::futures::Sink<SinkItem = <#proto_t as ::vessels::protocol::Protocol>::Call, SinkError = ()> {}
        ::vessels::macro_deps::lazy_static::lazy_static! {
            static ref EXPORT_CONCRETE_INSTANCE: ::std::sync::Mutex<Box<dyn EXPORT_CONCRETE_BOUND>> = {
                let ret: #self_ty = ::std::default::Default::default();
                let (sink, stream) = ret.into_protocol().split();
                let ret: Box<dyn EXPORT_CONCRETE_BOUND> = Box::new(sink);
                ::vessels::executor::spawn(stream.for_each(|item| {
                    let data = ::vessels::macro_deps::serde_cbor::to_vec(&item).unwrap();
                    unsafe { o(data.as_ptr(), data.len() as u32) };
                    Ok(())
                }));
                ::std::sync::Mutex::new(ret)
            };
        }
        extern "C" {
            fn o(ptr: *const u8, len: u32);
        }
        use ::vessels::macro_deps::futures::Stream as EXPORT_STREAM_USE;
        use ::vessels::protocol::Protocol as EXPORT_PROTOCOL_USE;
        #[no_mangle]
        pub extern "C" fn i(ptr: i32, len: i32) {
            let data: &'_ [u8] = unsafe { ::std::slice::from_raw_parts(ptr as _, len as _) };
            EXPORT_CONCRETE_INSTANCE.lock().unwrap().start_send(::vessels::macro_deps::serde_cbor::from_slice(data).unwrap()).unwrap();
        }
        #[no_mangle]
        pub static s: u64 = #proto_t::DO_NOT_IMPLEMENT_THIS_TRAIT_MANUALLY;
    };
    pfix.extend(proc_macro2::TokenStream::from(item));
    pfix.into()
}
