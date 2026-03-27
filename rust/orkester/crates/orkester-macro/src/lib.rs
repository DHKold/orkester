use proc_macro::TokenStream;
use proc_macro2::{Span, TokenTree};
use quote::quote;
use syn::{
    parse_macro_input,
    punctuated::Punctuated,
    Expr, ExprLit, GenericArgument, ImplItem, ImplItemFn, ItemImpl,
    Lit, LitStr, MetaNameValue, PathArguments, ReturnType, Token, Type,
};

// ── #[component(...)] ─────────────────────────────────────────────────────────

/// Derive [`PluginComponent`] for an impl block.
///
/// # Attributes
/// - `kind` (required) — fully-qualified kind identifier, e.g. `"example/Foo:1.0"`
/// - `name` (required) — human-readable name
/// - `description` (optional) — human-readable description
///
/// # Method annotations
/// - `#[handle("action/Kind")]` — register a typed request/response handler
/// - `#[factory("component/Kind")]` — register a sub-component factory
/// - `#[serializer(TypeName)]` — register a custom serializer for `TypeName`
/// - `#[deserializer("format/id")]` — register a custom deserializer for a format
///
/// # Example
/// ```ignore
/// #[component(kind = "example/Foo:1.0", name = "Foo")]
/// impl FooComponent {
///     #[handle("example/Do")]
///     fn do_it(&mut self, req: DoRequest) -> Result<DoResponse> { ... }
/// }
/// ```
#[proc_macro_attribute]
pub fn component(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args =
        parse_macro_input!(attr with Punctuated::<MetaNameValue, Token![,]>::parse_terminated);

    let meta = match parse_meta(args) {
        Ok(m) => m,
        Err(e) => return e.to_compile_error().into(),
    };

    // Strip a leading `pub` — `pub impl Foo { }` is a common style with this macro.
    let item2: proc_macro2::TokenStream = strip_leading_pub(item.into());
    let mut impl_block: ItemImpl = match syn::parse2(item2) {
        Ok(v) => v,
        Err(e) => return e.to_compile_error().into(),
    };

    let collected = match collect_methods(&mut impl_block) {
        Ok(v) => v,
        Err(e) => return e.to_compile_error().into(),
    };

    let self_ty = &impl_block.self_ty;
    let (impl_generics, _, where_clause) = impl_block.generics.split_for_impl();

    let ComponentMeta { kind, name, description } = &meta;

    // Build the .with_handler / .with_factory / .with_serializer / .with_deserializer chain.
    let builder_calls = builder_chain(&collected);

    let expanded = quote! {
        #impl_block

        impl #impl_generics ::orkester_plugin::sdk::PluginComponent for #self_ty #where_clause {
            fn get_metadata() -> ::orkester_plugin::sdk::ComponentMetadata {
                ::orkester_plugin::sdk::ComponentMetadata {
                    kind: #kind.to_string(),
                    name: #name.to_string(),
                    description: #description.to_string(),
                }
            }

            fn to_abi(self) -> ::orkester_plugin::abi::AbiComponent {
                ::orkester_plugin::sdk::AbiComponentBuilder::new()
                    .with_metadata(<Self as ::orkester_plugin::sdk::PluginComponent>::get_metadata())
                    #(#builder_calls)*
                    .build(self)
            }
        }
    };

    expanded.into()
}

// ── Internal data model ───────────────────────────────────────────────────────

struct ComponentMeta {
    kind: LitStr,
    name: LitStr,
    description: LitStr,
}

struct CollectedMethods {
    handlers: Vec<(Expr, ImplItemFn)>,
    factories: Vec<(LitStr, ImplItemFn)>,
    serializers: Vec<(proc_macro2::TokenStream, ImplItemFn)>,
    deserializers: Vec<(LitStr, ImplItemFn)>,
}

// ── Parsing helpers ───────────────────────────────────────────────────────────

fn parse_meta(args: Punctuated<MetaNameValue, Token![,]>) -> syn::Result<ComponentMeta> {
    let mut kind = None::<LitStr>;
    let mut name = None::<LitStr>;
    let mut description = None::<LitStr>;

    for arg in args {
        let key = arg.path.get_ident().map(|i| i.to_string()).unwrap_or_default();
        match key.as_str() {
            "kind" => kind = Some(expect_str(&arg.value)?),
            "name" => name = Some(expect_str(&arg.value)?),
            "description" => description = Some(expect_str(&arg.value)?),
            other => {
                return Err(syn::Error::new_spanned(
                    arg.path,
                    format!("unknown #[component] key `{other}`; expected kind, name, description"),
                ))
            }
        }
    }

    Ok(ComponentMeta {
        kind: kind.ok_or_else(|| {
            syn::Error::new(Span::call_site(), "#[component] requires `kind = \"...\"`")
        })?,
        name: name.ok_or_else(|| {
            syn::Error::new(Span::call_site(), "#[component] requires `name = \"...\"`")
        })?,
        description: description
            .unwrap_or_else(|| LitStr::new("", Span::call_site())),
    })
}

fn expect_str(expr: &Expr) -> syn::Result<LitStr> {
    match expr {
        Expr::Lit(ExprLit { lit: Lit::Str(s), .. }) => Ok(s.clone()),
        _ => Err(syn::Error::new_spanned(expr, "expected a string literal")),
    }
}

/// Walk every method in the impl block, strip known special attributes, and
/// collect the annotated methods into separate lists.
fn collect_methods(impl_block: &mut ItemImpl) -> syn::Result<CollectedMethods> {
    let mut handlers = Vec::new();
    let mut factories = Vec::new();
    let mut serializers = Vec::new();
    let mut deserializers = Vec::new();

    for item in &mut impl_block.items {
        let ImplItem::Fn(method) = item else { continue };

        let mut remaining_attrs = Vec::new();
        let mut handle_attr = None::<Expr>;
        let mut factory_attr = None::<LitStr>;
        let mut serializer_attr = None::<proc_macro2::TokenStream>;
        let mut deserializer_attr = None::<LitStr>;

        for attr in method.attrs.drain(..) {
            let path = attr.path();
            if path.is_ident("handle") {
                handle_attr = Some(attr.parse_args()?);
            } else if path.is_ident("factory") {
                factory_attr = Some(attr.parse_args()?);
            } else if path.is_ident("serializer") {
                serializer_attr = Some(attr.parse_args()?);
            } else if path.is_ident("deserializer") {
                deserializer_attr = Some(attr.parse_args()?);
            } else {
                remaining_attrs.push(attr);
            }
        }
        method.attrs = remaining_attrs;

        if let Some(action) = handle_attr {
            handlers.push((action, method.clone()));
        }
        if let Some(kind) = factory_attr {
            factories.push((kind, method.clone()));
        }
        if let Some(ty_ts) = serializer_attr {
            serializers.push((ty_ts, method.clone()));
        }
        if let Some(fmt) = deserializer_attr {
            deserializers.push((fmt, method.clone()));
        }
    }

    Ok(CollectedMethods { handlers, factories, serializers, deserializers })
}

/// Generate the chain of `.with_*(...)` calls for the `AbiComponentBuilder`.
fn builder_chain(m: &CollectedMethods) -> Vec<proc_macro2::TokenStream> {
    let mut calls = Vec::new();

    for (action, method) in &m.handlers {
        let name = &method.sig.ident;
        calls.push(quote! { .with_handler(#action, Self::#name) });
    }

    for (kind, method) in &m.factories {
        let name = &method.sig.ident;
        let sub_ty = match extract_result_ok_type(&method.sig.output) {
            Ok(t) => t.clone(),
            Err(e) => {
                // Emit a compile error token stream for this entry.
                let err = e.to_compile_error();
                calls.push(quote! { .with_factory(#kind, Self::#name, { #err; || unreachable!() }) });
                continue;
            }
        };
        calls.push(quote! {
            .with_factory(#kind, Self::#name, <#sub_ty as ::orkester_plugin::sdk::PluginComponent>::get_metadata)
        });
    }

    for (format_ty, method) in &m.serializers {
        let name = &method.sig.ident;
        let format_str = format_ty.to_string();
        calls.push(quote! {
            .with_serializer::<#format_ty>(#format_str, Self::#name)
        });
    }

    for (fmt, method) in &m.deserializers {
        let name = &method.sig.ident;
        calls.push(quote! { .with_deserializer::<_>(#fmt, Self::#name) });
    }

    calls
}

/// Extract the `T` from `Result<T>` or `Result<T, E>` in a return type.
fn extract_result_ok_type(ret: &ReturnType) -> syn::Result<&Type> {
    let ReturnType::Type(_, ty) = ret else {
        return Err(syn::Error::new(Span::call_site(), "factory method must return Result<T>"));
    };
    let Type::Path(path) = ty.as_ref() else {
        return Err(syn::Error::new_spanned(ty, "expected Result<T>"));
    };
    let last = path.path.segments.last()
        .ok_or_else(|| syn::Error::new_spanned(ty, "expected Result<T>"))?;
    if last.ident != "Result" {
        return Err(syn::Error::new_spanned(last, "factory return type must be Result<T>"));
    }
    let PathArguments::AngleBracketed(args) = &last.arguments else {
        return Err(syn::Error::new_spanned(last, "Result must have type arguments"));
    };
    match args.args.first() {
        Some(GenericArgument::Type(t)) => Ok(t),
        _ => Err(syn::Error::new_spanned(args, "Result must have a type argument")),
    }
}

/// Remove a leading `pub` keyword from a token stream, if present.
fn strip_leading_pub(ts: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    let mut iter = ts.into_iter().peekable();
    if let Some(TokenTree::Ident(id)) = iter.peek() {
        if id.to_string() == "pub" {
            iter.next();
        }
    }
    iter.collect()
}
