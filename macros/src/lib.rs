use proc_macro::TokenStream;

#[derive(Default, Debug, darling::FromMeta)]
#[darling(default)]
struct CommandArgs {
	no_dm: bool,
	description: Option<String>,
	default_member_permissions: Option<String>
}

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

fn wrap_option_to_string<T: quote::ToTokens>(literal: Option<T>) -> syn::Expr {
    match literal {
        Some(literal) => syn::parse_quote! { Some(#literal.to_string()) },
        None => syn::parse_quote! { None },
    }
}

#[proc_macro_attribute]
pub fn command(args: TokenStream, function: TokenStream) -> TokenStream {
	let args = match darling::ast::NestedMeta::parse_meta_list(args.into()) {
        Ok(x) => x,
        Err(e) => return e.into_compile_error().into(),
    };

    let args = match <CommandArgs as darling::FromMeta>::from_list(&args) {
        Ok(x) => x,
        Err(e) => return e.write_errors().into(),
    };

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

	let no_dm = args.no_dm;
	let description = wrap_option_to_string(args.description);
	let default_member_permissions = wrap_option_to_string(args.default_member_permissions);
	TokenStream::from(quote::quote! {
		#function_visibility fn #function_ident #function_generics() -> crate::Command {
            #function
			crate::Command {
				name: #function_name,
				no_dm: #no_dm,
				description: #description,
				slash_action: #slash,
				default_member_permissions: #default_member_permissions
			}
		}
	})
}