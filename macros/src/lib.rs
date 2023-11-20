use proc_macro::TokenStream;

fn create_slash() -> proc_macro2::TokenStream {
	quote::quote! {
		|interaction| Box::pin(async move {
			inner(interaction).await
		})
	}
}

fn wrap_option<T: quote::ToTokens>(literal: Option<T>) -> syn::Expr {
    match literal {
        Some(literal) => syn::parse_quote! { Some(#literal) },
        None => syn::parse_quote! { None },
    }
}

#[proc_macro_attribute]
pub fn command(_args: TokenStream, function: TokenStream) -> TokenStream {
	let mut function = syn::parse_macro_input!(function as syn::ItemFn);

	let function_name = function
        .sig
        .ident
        .to_string()
        .trim_start_matches("r#")
        .to_string();

	let function_ident =
        std::mem::replace(&mut function.sig.ident, syn::parse_quote! { inner });
    let function_generics = &function.sig.generics;
    let function_visibility = &function.vis;
    let function = &function;

	let slash = wrap_option(Some(create_slash()));
	TokenStream::from(quote::quote! {
		#function_visibility fn #function_ident #function_generics() -> crate::Command {
            #function
			crate::Command {
				name: #function_name,
				slash_action: #slash
			}
		}
	})
}