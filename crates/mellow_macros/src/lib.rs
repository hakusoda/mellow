use syn::spanned::Spanned;
use darling::FromMeta;
use proc_macro::TokenStream;

#[derive(Debug, Default, FromMeta)]
#[darling(default)]
struct CommandArgs {
	user: bool,
	no_dm: bool,
	slash: bool,
	rename: Option<String>,
	message: bool,
	description: Option<String>,
	default_member_permissions: Option<String>
}

fn create_handler() -> proc_macro2::TokenStream {
	quote::quote! {
		|context, interaction| Box::pin(async move {
			inner(context, interaction).await
		})
	}
}

fn wrap_option_to_string<T: quote::ToTokens>(literal: Option<T>) -> syn::Expr {
    match literal {
        Some(literal) => syn::parse_quote! { Some(#literal.to_string()) },
        None => syn::parse_quote! { None },
    }
}

fn create_command(args: TokenStream, mut function: syn::ItemFn) -> Result<TokenStream, darling::Error> {
	let args = darling::ast::NestedMeta::parse_meta_list(args.into())?;
    let args = <CommandArgs as darling::FromMeta>::from_list(&args)?;
	if !args.user && !args.slash && !args.message {
		return Err(syn::Error::new(function.sig.span(), "command must specify either user, slash, or message").into());
	}

	let function_name = function
        .sig
        .ident
        .to_string()
        .trim_start_matches("r#")
        .to_string();

	let function_ident = std::mem::replace(&mut function.sig.ident, syn::parse_quote! { inner });
    let function_generics = &function.sig.generics;
    let function_visibility = &function.vis;
    let function = &function;

	let handler = create_handler();

	let no_dm = args.no_dm;
	let rename = wrap_option_to_string(args.rename);
	let is_user = args.user;
	let is_slash = args.slash;
	let is_message = args.message;
	let description = wrap_option_to_string(args.description);
	let default_member_permissions = wrap_option_to_string(args.default_member_permissions);
	Ok(TokenStream::from(quote::quote! {
		#function_visibility fn #function_ident #function_generics() -> crate::Command {
            #function
			crate::Command {
				name: #rename.unwrap_or(#function_name.to_string()),
				no_dm: #no_dm,
				handler: #handler,
				is_user: #is_user,
				is_slash: #is_slash,
				is_message: #is_message,
				description: #description,
				default_member_permissions: #default_member_permissions
			}
		}
	}))
}

#[proc_macro_attribute]
pub fn command(args: TokenStream, function: TokenStream) -> TokenStream {
	let function = syn::parse_macro_input!(function as syn::ItemFn);
	match create_command(args, function) {
		Ok(x) => x,
		Err(x) => x.write_errors().into()
	}
}