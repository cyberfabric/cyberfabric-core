#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
use heck::ToSnakeCase;
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, quote};
use syn::{
    DeriveInput, Expr, Ident, ImplItem, ItemImpl, Lit, LitBool, LitStr, Meta, MetaList,
    MetaNameValue, Path, Token, TypePath, parse::Parse, parse::ParseStream, parse_macro_input,
    punctuated::Punctuated,
};

mod api_dto;
mod domain_model;
mod grpc_client;
mod utils;

/// Configuration parsed from #[module(...)] attribute
struct ModuleConfig {
    name: String,
    deps: Vec<String>,
    caps: Vec<Capability>,
    ctor: Option<Expr>,             // arbitrary constructor expression
    client: Option<Path>,           // trait path for client DX helpers
    lifecycle: Option<LcModuleCfg>, // optional lifecycle config (on type)
}

#[derive(Debug, PartialEq, Clone)]
enum Capability {
    Db,
    Rest,
    RestHost,
    Stateful,
    System,
    GrpcHub,
    Grpc,
}

impl Capability {
    const VALID_CAPABILITIES: &'static [&'static str] = &[
        "db",
        "rest",
        "rest_host",
        "stateful",
        "system",
        "grpc_hub",
        "grpc",
    ];

    fn suggest_similar(input: &str) -> Vec<&'static str> {
        let mut suggestions: Vec<(&str, f64)> = Self::VALID_CAPABILITIES
            .iter()
            .map(|&cap| (cap, strsim::jaro_winkler(input, cap)))
            .filter(|(_, score)| *score > 0.6) // Only suggest if reasonably similar
            .collect();

        suggestions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        suggestions
            .into_iter()
            .take(2)
            .map(|(cap, _)| cap)
            .collect()
    }

    fn from_ident(ident: &Ident) -> syn::Result<Self> {
        let input = ident.to_string();
        match input.as_str() {
            "db" => Ok(Capability::Db),
            "rest" => Ok(Capability::Rest),
            "rest_host" => Ok(Capability::RestHost),
            "stateful" => Ok(Capability::Stateful),
            "system" => Ok(Capability::System),
            "grpc_hub" => Ok(Capability::GrpcHub),
            "grpc" => Ok(Capability::Grpc),
            other => {
                let suggestions = Self::suggest_similar(other);
                let error_msg = if suggestions.is_empty() {
                    format!(
                        "unknown capability '{other}', expected one of: db, rest, rest_host, stateful, system, grpc_hub, grpc"
                    )
                } else {
                    format!(
                        "unknown capability '{other}'\n       = help: did you mean one of: {}?",
                        suggestions.join(", ")
                    )
                };
                Err(syn::Error::new_spanned(ident, error_msg))
            }
        }
    }

    fn from_str_lit(lit: &LitStr) -> syn::Result<Self> {
        let input = lit.value();
        match input.as_str() {
            "db" => Ok(Capability::Db),
            "rest" => Ok(Capability::Rest),
            "rest_host" => Ok(Capability::RestHost),
            "stateful" => Ok(Capability::Stateful),
            "system" => Ok(Capability::System),
            "grpc_hub" => Ok(Capability::GrpcHub),
            "grpc" => Ok(Capability::Grpc),
            other => {
                let suggestions = Self::suggest_similar(other);
                let error_msg = if suggestions.is_empty() {
                    format!(
                        "unknown capability '{other}', expected one of: db, rest, rest_host, stateful, system, grpc_hub, grpc"
                    )
                } else {
                    format!(
                        "unknown capability '{other}'\n       = help: did you mean one of: {}?",
                        suggestions.join(", ")
                    )
                };
                Err(syn::Error::new_spanned(lit, error_msg))
            }
        }
    }
}

/// Validates that a module name follows kebab-case naming convention.
///
/// # Rules
/// - Must contain only lowercase letters (a-z), digits (0-9), and hyphens (-)
/// - Must start with a lowercase letter
/// - Must not end with a hyphen
/// - Must not contain consecutive hyphens
/// - Must not contain underscores (use hyphens instead)
///
/// # Examples
/// Valid: "file-parser", "api-gateway", "simple-user-settings", "types-registry"
/// Invalid: "`file_parser`" (underscores), "`FileParser`" (uppercase), "-parser" (starts with hyphen)
fn validate_kebab_case(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("module name cannot be empty".to_owned());
    }

    // Check for underscores (common mistake)
    if name.contains('_') {
        let suggested = name.replace('_', "-");
        return Err(format!(
            "module name must use kebab-case, not snake_case\n       = help: use '{suggested}' instead of '{name}'"
        ));
    }

    // Must start with a lowercase letter
    if let Some(first_char) = name.chars().next() {
        if !first_char.is_ascii_lowercase() {
            return Err(format!(
                "module name must start with a lowercase letter, found '{first_char}'"
            ));
        }
    } else {
        // This should never happen due to the empty check above
        return Err("module name cannot be empty".to_owned());
    }

    // Must not end with hyphen
    if name.ends_with('-') {
        return Err("module name must not end with a hyphen".to_owned());
    }

    // Check for invalid characters and consecutive hyphens
    let mut prev_was_hyphen = false;
    for ch in name.chars() {
        if ch == '-' {
            if prev_was_hyphen {
                return Err("module name must not contain consecutive hyphens".to_owned());
            }
            prev_was_hyphen = true;
        } else if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            prev_was_hyphen = false;
        } else {
            return Err(format!(
                "module name must contain only lowercase letters, digits, and hyphens, found '{ch}'"
            ));
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct LcModuleCfg {
    entry: String,        // entry method name (e.g., "serve")
    stop_timeout: String, // human duration (e.g., "30s")
    await_ready: bool,    // require ReadySignal gating
}

impl Default for LcModuleCfg {
    fn default() -> Self {
        Self {
            entry: "serve".to_owned(),
            stop_timeout: "30s".to_owned(),
            await_ready: false,
        }
    }
}

impl Parse for ModuleConfig {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut name: Option<String> = None;
        let mut deps: Vec<String> = Vec::new();
        let mut caps: Vec<Capability> = Vec::new();
        let mut ctor: Option<Expr> = None;
        let mut client: Option<Path> = None;
        let mut lifecycle: Option<LcModuleCfg> = None;

        let mut seen_name = false;
        let mut seen_deps = false;
        let mut seen_caps = false;
        let mut seen_ctor = false;
        let mut seen_client = false;
        let mut seen_lifecycle = false;

        let punctuated: Punctuated<Meta, Token![,]> =
            input.parse_terminated(Meta::parse, Token![,])?;

        for meta in punctuated {
            match meta {
                Meta::NameValue(nv) if nv.path.is_ident("name") => {
                    if seen_name {
                        return Err(syn::Error::new_spanned(
                            nv.path,
                            "duplicate `name` parameter",
                        ));
                    }
                    seen_name = true;
                    match nv.value {
                        Expr::Lit(syn::ExprLit {
                            lit: Lit::Str(s), ..
                        }) => {
                            let module_name = s.value();
                            // Validate kebab-case format
                            if let Err(err) = validate_kebab_case(&module_name) {
                                return Err(syn::Error::new_spanned(s, err));
                            }
                            name = Some(module_name);
                        }
                        other => {
                            return Err(syn::Error::new_spanned(
                                other,
                                "name must be a string literal, e.g. name = \"my-module\"",
                            ));
                        }
                    }
                }
                Meta::NameValue(nv) if nv.path.is_ident("ctor") => {
                    if seen_ctor {
                        return Err(syn::Error::new_spanned(
                            nv.path,
                            "duplicate `ctor` parameter",
                        ));
                    }
                    seen_ctor = true;

                    // Reject string literals with a clear message.
                    match &nv.value {
                        Expr::Lit(syn::ExprLit {
                            lit: Lit::Str(s), ..
                        }) => {
                            return Err(syn::Error::new_spanned(
                                s,
                                "ctor must be a Rust expression, not a string literal. \
                 Use: ctor = MyType::new()  (with parentheses), \
                 or:  ctor = Default::default()",
                            ));
                        }
                        _ => {
                            ctor = Some(nv.value.clone());
                        }
                    }
                }
                Meta::NameValue(nv) if nv.path.is_ident("client") => {
                    if seen_client {
                        return Err(syn::Error::new_spanned(
                            nv.path,
                            "duplicate `client` parameter",
                        ));
                    }
                    seen_client = true;
                    let value = nv.value.clone();
                    match value {
                        Expr::Path(ep) => {
                            client = Some(ep.path);
                        }
                        other => {
                            return Err(syn::Error::new_spanned(
                                other,
                                "client must be a trait path, e.g. client = crate::api::MyClient",
                            ));
                        }
                    }
                }
                Meta::NameValue(nv) if nv.path.is_ident("deps") => {
                    if seen_deps {
                        return Err(syn::Error::new_spanned(
                            nv.path,
                            "duplicate `deps` parameter",
                        ));
                    }
                    seen_deps = true;
                    let value = nv.value.clone();
                    match value {
                        Expr::Array(arr) => {
                            for elem in arr.elems {
                                match elem {
                                    Expr::Lit(syn::ExprLit {
                                        lit: Lit::Str(s), ..
                                    }) => {
                                        deps.push(s.value());
                                    }
                                    other => {
                                        return Err(syn::Error::new_spanned(
                                            other,
                                            "deps must be an array of string literals, e.g. deps = [\"db\", \"auth\"]",
                                        ));
                                    }
                                }
                            }
                        }
                        other => {
                            return Err(syn::Error::new_spanned(
                                other,
                                "deps must be an array, e.g. deps = [\"db\", \"auth\"]",
                            ));
                        }
                    }
                }
                Meta::NameValue(nv) if nv.path.is_ident("capabilities") => {
                    if seen_caps {
                        return Err(syn::Error::new_spanned(
                            nv.path,
                            "duplicate `capabilities` parameter",
                        ));
                    }
                    seen_caps = true;
                    let value = nv.value.clone();
                    match value {
                        Expr::Array(arr) => {
                            for elem in arr.elems {
                                match elem {
                                    Expr::Path(ref path) => {
                                        if let Some(ident) = path.path.get_ident() {
                                            caps.push(Capability::from_ident(ident)?);
                                        } else {
                                            return Err(syn::Error::new_spanned(
                                                path,
                                                "capability must be a simple identifier (db, rest, rest_host, stateful)",
                                            ));
                                        }
                                    }
                                    Expr::Lit(syn::ExprLit {
                                        lit: Lit::Str(s), ..
                                    }) => {
                                        caps.push(Capability::from_str_lit(&s)?);
                                    }
                                    other => {
                                        return Err(syn::Error::new_spanned(
                                            other,
                                            "capability must be an identifier or string literal (\"db\", \"rest\", \"rest_host\", \"stateful\")",
                                        ));
                                    }
                                }
                            }
                        }
                        other => {
                            return Err(syn::Error::new_spanned(
                                other,
                                "capabilities must be an array, e.g. capabilities = [db, rest]",
                            ));
                        }
                    }
                }
                // Accept `lifecycle(...)` and also namespaced like `modkit::module::lifecycle(...)`
                Meta::List(list) if path_last_is(&list.path, "lifecycle") => {
                    if seen_lifecycle {
                        return Err(syn::Error::new_spanned(
                            list.path,
                            "duplicate `lifecycle(...)` parameter",
                        ));
                    }
                    seen_lifecycle = true;
                    lifecycle = Some(parse_lifecycle_list(&list)?);
                }
                other => {
                    return Err(syn::Error::new_spanned(
                        other,
                        "unknown attribute parameter",
                    ));
                }
            }
        }

        let name = name.ok_or_else(|| {
            syn::Error::new(
                Span::call_site(),
                "name parameter is required, e.g. #[module(name = \"my-module\", ...)]",
            )
        })?;

        Ok(ModuleConfig {
            name,
            deps,
            caps,
            ctor,
            client,
            lifecycle,
        })
    }
}

fn parse_lifecycle_list(list: &MetaList) -> syn::Result<LcModuleCfg> {
    let mut cfg = LcModuleCfg::default();

    let inner: Punctuated<Meta, Token![,]> =
        list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;

    for m in inner {
        match m {
            Meta::NameValue(MetaNameValue { path, value, .. }) if path.is_ident("entry") => {
                if let Expr::Lit(syn::ExprLit {
                    lit: Lit::Str(s), ..
                }) = value
                {
                    cfg.entry = s.value();
                } else {
                    return Err(syn::Error::new_spanned(
                        value,
                        "entry must be a string literal, e.g. entry = \"serve\"",
                    ));
                }
            }
            Meta::NameValue(MetaNameValue { path, value, .. }) if path.is_ident("stop_timeout") => {
                if let Expr::Lit(syn::ExprLit {
                    lit: Lit::Str(s), ..
                }) = value
                {
                    cfg.stop_timeout = s.value();
                } else {
                    return Err(syn::Error::new_spanned(
                        value,
                        "stop_timeout must be a string literal like \"45s\"",
                    ));
                }
            }
            Meta::Path(p) if p.is_ident("await_ready") => {
                cfg.await_ready = true;
            }
            Meta::NameValue(MetaNameValue { path, value, .. }) if path.is_ident("await_ready") => {
                if let Expr::Lit(syn::ExprLit {
                    lit: Lit::Bool(LitBool { value: b, .. }),
                    ..
                }) = value
                {
                    cfg.await_ready = b;
                } else {
                    return Err(syn::Error::new_spanned(
                        value,
                        "await_ready must be a bool literal (true/false) or a bare flag",
                    ));
                }
            }
            other => {
                return Err(syn::Error::new_spanned(
                    other,
                    "expected lifecycle args: entry=\"...\", stop_timeout=\"...\", await_ready[=true|false]",
                ));
            }
        }
    }

    Ok(cfg)
}

/// Main #[module] attribute macro
///
/// `ctor` must be a Rust expression that evaluates to the module instance,
/// e.g. `ctor = MyModule::new()` or `ctor = Default::default()`.
#[proc_macro_attribute]
#[allow(clippy::too_many_lines)]
pub fn module(attr: TokenStream, item: TokenStream) -> TokenStream {
    let config = parse_macro_input!(attr as ModuleConfig);
    let input = parse_macro_input!(item as DeriveInput);

    // --- Clone all needed pieces early to avoid use-after-move issues ---
    let struct_ident = input.ident.clone();
    let generics_clone = input.generics.clone();
    let (impl_generics, ty_generics, where_clause) = generics_clone.split_for_impl();

    let name_owned: String = config.name.clone();
    let deps_owned: Vec<String> = config.deps.clone();
    let caps_for_asserts: Vec<Capability> = config.caps.clone();
    let caps_for_regs: Vec<Capability> = config.caps.clone();
    let ctor_expr_opt: Option<Expr> = config.ctor.clone();
    let client_trait_opt: Option<Path> = config.client.clone();
    let lifecycle_cfg_opt: Option<LcModuleCfg> = config.lifecycle;

    // Prepare string literals for name/deps
    let name_lit = LitStr::new(&name_owned, Span::call_site());
    let deps_lits: Vec<LitStr> = deps_owned
        .iter()
        .map(|s| LitStr::new(s, Span::call_site()))
        .collect();

    // Constructor expression (provided or Default::default())
    let constructor = if let Some(expr) = &ctor_expr_opt {
        quote! { #expr }
    } else {
        // Use `<T as Default>::default()` so generics/where-clause are honored.
        quote! { <#struct_ident #ty_generics as ::core::default::Default>::default() }
    };

    // Compile-time capability assertions (no calls in consts)
    let mut cap_asserts = Vec::new();

    // Always assert Module is implemented
    cap_asserts.push(quote! {
        const _: () = {
            #[allow(dead_code)]
            fn __modkit_require_Module_impl()
            where
                #struct_ident #ty_generics: ::modkit::contracts::Module,
            {}
        };
    });

    for cap in &caps_for_asserts {
        let q = match cap {
            Capability::Db => quote! {
                const _: () = {
                    #[allow(dead_code)]
                    fn __modkit_require_DatabaseCapability_impl()
                    where
                        #struct_ident #ty_generics: ::modkit::contracts::DatabaseCapability,
                    {}
                };
            },
            Capability::Rest => quote! {
                const _: () = {
                    #[allow(dead_code)]
                    fn __modkit_require_RestApiCapability_impl()
                    where
                        #struct_ident #ty_generics: ::modkit::contracts::RestApiCapability,
                    {}
                };
            },
            Capability::RestHost => quote! {
                const _: () = {
                    #[allow(dead_code)]
                    fn __modkit_require_ApiGatewayCapability_impl()
                    where
                        #struct_ident #ty_generics: ::modkit::contracts::ApiGatewayCapability,
                    {}
                };
            },
            Capability::Stateful => {
                if lifecycle_cfg_opt.is_none() {
                    // Only require direct RunnableCapability impl when lifecycle(...) is NOT used.
                    quote! {
                        const _: () = {
                            #[allow(dead_code)]
                            fn __modkit_require_RunnableCapability_impl()
                            where
                                #struct_ident #ty_generics: ::modkit::contracts::RunnableCapability,
                            {}
                        };
                    }
                } else {
                    quote! {}
                }
            }
            Capability::System => {
                // System is a flag, no trait required
                quote! {}
            }
            Capability::GrpcHub => quote! {
                const _: () = {
                    #[allow(dead_code)]
                    fn __modkit_require_GrpcHubCapability_impl()
                    where
                        #struct_ident #ty_generics: ::modkit::contracts::GrpcHubCapability,
                    {}
                };
            },
            Capability::Grpc => quote! {
                const _: () = {
                    #[allow(dead_code)]
                    fn __modkit_require_GrpcServiceCapability_impl()
                    where
                        #struct_ident #ty_generics: ::modkit::contracts::GrpcServiceCapability,
                    {}
                };
            },
        };
        cap_asserts.push(q);
    }

    // Registrator name (avoid lowercasing to reduce collisions)
    let struct_name_snake = struct_ident.to_string().to_snake_case();
    let registrator_name = format_ident!("__{}_registrator", struct_name_snake);

    // === Top-level extras (impl Runnable + optional ready shim) ===
    let mut extra_top_level = proc_macro2::TokenStream::new();

    if let Some(lc) = &lifecycle_cfg_opt {
        // If the type declares lifecycle(...), we generate Runnable at top-level.
        let entry_ident = format_ident!("{}", lc.entry);
        let timeout_ts =
            parse_duration_tokens(&lc.stop_timeout).unwrap_or_else(|e| e.to_compile_error());
        let await_ready_bool = lc.await_ready;

        if await_ready_bool {
            let ready_shim_ident =
                format_ident!("__modkit_run_ready_shim_for_{}", struct_name_snake);

            // Runnable calls entry(cancel, ready). Shim is used by WithLifecycle in ready mode.
            extra_top_level.extend(quote! {
                #[::async_trait::async_trait]
                impl #impl_generics ::modkit::lifecycle::Runnable for #struct_ident #ty_generics #where_clause {
                    async fn run(self: ::std::sync::Arc<Self>, cancel: ::tokio_util::sync::CancellationToken) -> ::anyhow::Result<()> {
                        let (_tx, _rx) = ::tokio::sync::oneshot::channel::<()>();
                        let ready = ::modkit::lifecycle::ReadySignal::from_sender(_tx);
                        self.#entry_ident(cancel, ready).await
                    }
                }

                #[doc(hidden)]
                #[allow(dead_code, non_snake_case)]
                fn #ready_shim_ident(
                    this: ::std::sync::Arc<#struct_ident #ty_generics>,
                    cancel: ::tokio_util::sync::CancellationToken,
                    ready: ::modkit::lifecycle::ReadySignal,
                ) -> ::core::pin::Pin<Box<dyn ::core::future::Future<Output = ::anyhow::Result<()>> + Send>> {
                    Box::pin(async move { this.#entry_ident(cancel, ready).await })
                }
            });

            // Convenience `into_module()` API.
            extra_top_level.extend(quote! {
                impl #impl_generics #struct_ident #ty_generics #where_clause {
                    /// Wrap this instance into a stateful module with lifecycle configuration.
                    pub fn into_module(self) -> ::modkit::lifecycle::WithLifecycle<Self> {
                        ::modkit::lifecycle::WithLifecycle::new_with_name(self, #name_lit)
                            .with_stop_timeout(#timeout_ts)
                            .with_ready_mode(true, true, Some(#ready_shim_ident))
                    }
                }
            });
        } else {
            // No ready gating: Runnable calls entry(cancel).
            extra_top_level.extend(quote! {
                #[::async_trait::async_trait]
                impl #impl_generics ::modkit::lifecycle::Runnable for #struct_ident #ty_generics #where_clause {
                    async fn run(self: ::std::sync::Arc<Self>, cancel: ::tokio_util::sync::CancellationToken) -> ::anyhow::Result<()> {
                        self.#entry_ident(cancel).await
                    }
                }

                impl #impl_generics #struct_ident #ty_generics #where_clause {
                    /// Wrap this instance into a stateful module with lifecycle configuration.
                    pub fn into_module(self) -> ::modkit::lifecycle::WithLifecycle<Self> {
                        ::modkit::lifecycle::WithLifecycle::new_with_name(self, #name_lit)
                            .with_stop_timeout(#timeout_ts)
                            .with_ready_mode(false, false, None)
                    }
                }
            });
        }
    }

    // Capability registrations (builder API), with special handling for stateful + lifecycle
    let capability_registrations = caps_for_regs.iter().map(|cap| {
        match cap {
            Capability::Db => quote! {
                b.register_db_with_meta(#name_lit,
                    module.clone() as ::std::sync::Arc<dyn ::modkit::contracts::DatabaseCapability>);
            },
            Capability::Rest => quote! {
                b.register_rest_with_meta(#name_lit,
                    module.clone() as ::std::sync::Arc<dyn ::modkit::contracts::RestApiCapability>);
            },
            Capability::RestHost => quote! {
                b.register_rest_host_with_meta(#name_lit,
                    module.clone() as ::std::sync::Arc<dyn ::modkit::contracts::ApiGatewayCapability>);
            },
            Capability::Stateful => {
                if let Some(lc) = &lifecycle_cfg_opt {
                    let timeout_ts = parse_duration_tokens(&lc.stop_timeout)
                        .unwrap_or_else(|e| e.to_compile_error());
                    let await_ready_bool = lc.await_ready;
                    let ready_shim_ident =
                        format_ident!("__modkit_run_ready_shim_for_{}", struct_name_snake);

                    if await_ready_bool {
                        quote! {
                            let wl = ::modkit::lifecycle::WithLifecycle::from_arc_with_name(
                                    module.clone(),
                                    #name_lit,
                                )
                                .with_stop_timeout(#timeout_ts)
                                .with_ready_mode(true, true, Some(#ready_shim_ident));

                            b.register_stateful_with_meta(
                                #name_lit,
                                ::std::sync::Arc::new(wl) as ::std::sync::Arc<dyn ::modkit::contracts::RunnableCapability>
                            );
                        }
                    } else {
                        quote! {
                            let wl = ::modkit::lifecycle::WithLifecycle::from_arc_with_name(
                                    module.clone(),
                                    #name_lit,
                                )
                                .with_stop_timeout(#timeout_ts)
                                .with_ready_mode(false, false, None);

                            b.register_stateful_with_meta(
                                #name_lit,
                                ::std::sync::Arc::new(wl) as ::std::sync::Arc<dyn ::modkit::contracts::RunnableCapability>
                            );
                        }
                    }
                } else {
                    // Alternative path: the type itself must implement RunnableCapability
                    quote! {
                        b.register_stateful_with_meta(#name_lit,
                            module.clone() as ::std::sync::Arc<dyn ::modkit::contracts::RunnableCapability>);
                    }
                }
            },
            Capability::System => quote! {
                b.register_system_with_meta(#name_lit,
                    module.clone() as ::std::sync::Arc<dyn ::modkit::contracts::SystemCapability>);
            },
            Capability::GrpcHub => quote! {
                b.register_grpc_hub_with_meta(#name_lit,
                    module.clone() as ::std::sync::Arc<dyn ::modkit::contracts::GrpcHubCapability>);
            },
            Capability::Grpc => quote! {
                b.register_grpc_service_with_meta(#name_lit,
                    module.clone() as ::std::sync::Arc<dyn ::modkit::contracts::GrpcServiceCapability>);
            },
        }
    });

    // ClientHub DX helpers (optional)
    // Note: The `client` parameter now only triggers compile-time trait checks.
    // For client registration/access, use `hub.register::<dyn Trait>(client)` and
    // `hub.get::<dyn Trait>()` directly, or provide helpers in your *-sdk crate.
    let client_code = if let Some(client_trait_path) = &client_trait_opt {
        quote! {
            // Compile-time trait checks: object-safe + Send + Sync + 'static
            const _: () = {
                fn __modkit_obj_safety<T: ?Sized + ::core::marker::Send + ::core::marker::Sync + 'static>() {}
                let _ = __modkit_obj_safety::<dyn #client_trait_path> as fn();
            };

            impl #impl_generics #struct_ident #ty_generics #where_clause {
                pub const MODULE_NAME: &'static str = #name_lit;
            }
        }
    } else {
        // Even without a client trait, expose MODULE_NAME for ergonomics.
        quote! {
            impl #impl_generics #struct_ident #ty_generics #where_clause {
                pub const MODULE_NAME: &'static str = #name_lit;
            }
        }
    };

    // Final expansion:
    let expanded = quote! {
        #input

        // Compile-time capability assertions (better errors if trait impls are missing)
        #(#cap_asserts)*

        // Registrator that targets the *builder*, not the final registry
        #[doc(hidden)]
        fn #registrator_name(b: &mut ::modkit::registry::RegistryBuilder) {
            use ::std::sync::Arc;

            let module: Arc<#struct_ident #ty_generics> = Arc::new(#constructor);

            // register core with metadata (name + deps)
            b.register_core_with_meta(
                #name_lit,
                &[#(#deps_lits),*],
                module.clone() as Arc<dyn ::modkit::contracts::Module>
            );

            // capabilities
            #(#capability_registrations)*
        }

        ::modkit::inventory::submit! {
            ::modkit::registry::Registrator(#registrator_name)
        }

        #client_code

        // Top-level extras for lifecycle-enabled types (impl Runnable, ready shim, into_module)
        #extra_top_level
    };

    TokenStream::from(expanded)
}

// ============================================================================
// Lifecycle Macro (impl-block attribute) — still supported for opt-in usage
// ============================================================================

#[derive(Debug)]
struct LcCfg {
    method: String,
    stop_timeout: String,
    await_ready: bool,
}

#[proc_macro_attribute]
pub fn lifecycle(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr with Punctuated::<Meta, Token![,]>::parse_terminated);
    let impl_item = parse_macro_input!(item as ItemImpl);

    let cfg = match parse_lifecycle_args(args) {
        Ok(c) => c,
        Err(e) => return e.to_compile_error().into(),
    };

    // Extract impl type ident
    let ty = match &*impl_item.self_ty {
        syn::Type::Path(TypePath { path, .. }) => path.clone(),
        other => {
            return syn::Error::new_spanned(other, "unsupported impl target")
                .to_compile_error()
                .into();
        }
    };

    let runner_ident = format_ident!("{}", cfg.method);
    let mut has_runner = false;
    let mut takes_ready_signal = false;
    for it in &impl_item.items {
        if let ImplItem::Fn(f) = it
            && f.sig.ident == runner_ident
        {
            has_runner = true;
            if f.sig.asyncness.is_none() {
                return syn::Error::new_spanned(f.sig.fn_token, "runner must be async")
                    .to_compile_error()
                    .into();
            }
            let input_count = f.sig.inputs.len();
            match input_count {
                2 => {}
                3 => {
                    if let Some(syn::FnArg::Typed(pat_ty)) = f.sig.inputs.iter().nth(2) {
                        match &*pat_ty.ty {
                            syn::Type::Path(tp) => {
                                if let Some(seg) = tp.path.segments.last() {
                                    if seg.ident == "ReadySignal" {
                                        takes_ready_signal = true;
                                    } else {
                                        return syn::Error::new_spanned(
                                            &pat_ty.ty,
                                            "third parameter must be ReadySignal when await_ready=true",
                                        )
                                            .to_compile_error()
                                            .into();
                                    }
                                }
                            }
                            other => {
                                return syn::Error::new_spanned(
                                    other,
                                    "third parameter must be ReadySignal when await_ready=true",
                                )
                                .to_compile_error()
                                .into();
                            }
                        }
                    }
                }
                _ => {
                    return syn::Error::new_spanned(
                        f.sig.inputs.clone(),
                        "invalid runner signature; expected (&self, CancellationToken) or (&self, CancellationToken, ReadySignal)",
                    )
                        .to_compile_error()
                        .into();
                }
            }
        }
    }
    if !has_runner {
        return syn::Error::new(
            Span::call_site(),
            format!("runner method `{}` not found in impl", cfg.method),
        )
        .to_compile_error()
        .into();
    }

    // Duration literal token
    let timeout_ts = match parse_duration_tokens(&cfg.stop_timeout) {
        Ok(ts) => ts,
        Err(e) => return e.to_compile_error().into(),
    };

    // Generated additions (outside of impl-block)
    let ty_ident = match ty.segments.last() {
        Some(seg) => seg.ident.clone(),
        None => {
            return syn::Error::new_spanned(
                &ty,
                "unsupported impl target: expected a concrete type path",
            )
            .to_compile_error()
            .into();
        }
    };
    let ty_snake = ty_ident.to_string().to_snake_case();

    let ready_shim_ident = format_ident!("__modkit_run_ready_shim{ty_snake}");
    let await_ready_bool = cfg.await_ready;

    let extra = if takes_ready_signal {
        quote! {
            #[async_trait::async_trait]
            impl ::modkit::lifecycle::Runnable for #ty {
                async fn run(self: ::std::sync::Arc<Self>, cancel: ::tokio_util::sync::CancellationToken) -> ::anyhow::Result<()> {
                    let (_tx, _rx) = ::tokio::sync::oneshot::channel::<()>();
                    let ready = ::modkit::lifecycle::ReadySignal::from_sender(_tx);
                    self.#runner_ident(cancel, ready).await
                }
            }

            #[doc(hidden)]
            #[allow(non_snake_case, dead_code)]
            fn #ready_shim_ident(
                this: ::std::sync::Arc<#ty>,
                cancel: ::tokio_util::sync::CancellationToken,
                ready: ::modkit::lifecycle::ReadySignal,
            ) -> ::core::pin::Pin<Box<dyn ::core::future::Future<Output = ::anyhow::Result<()>> + Send>> {
                Box::pin(async move { this.#runner_ident(cancel, ready).await })
            }

            impl #ty {
                /// Converts this value into a stateful module wrapper with configured stop-timeout.
                pub fn into_module(self) -> ::modkit::lifecycle::WithLifecycle<Self> {
                    ::modkit::lifecycle::WithLifecycle::new(self)
                        .with_stop_timeout(#timeout_ts)
                        .with_ready_mode(#await_ready_bool, true, Some(#ready_shim_ident))
                }
            }
        }
    } else {
        quote! {
            #[async_trait::async_trait]
            impl ::modkit::lifecycle::Runnable for #ty {
                async fn run(self: ::std::sync::Arc<Self>, cancel: ::tokio_util::sync::CancellationToken) -> ::anyhow::Result<()> {
                    self.#runner_ident(cancel).await
                }
            }

            impl #ty {
                /// Converts this value into a stateful module wrapper with configured stop-timeout.
                pub fn into_module(self) -> ::modkit::lifecycle::WithLifecycle<Self> {
                    ::modkit::lifecycle::WithLifecycle::new(self)
                        .with_stop_timeout(#timeout_ts)
                        .with_ready_mode(#await_ready_bool, false, None)
                }
            }
        }
    };

    let out = quote! {
        #impl_item
        #extra
    };
    out.into()
}

fn parse_lifecycle_args(args: Punctuated<Meta, Token![,]>) -> syn::Result<LcCfg> {
    let mut method: Option<String> = None;
    let mut stop_timeout = "30s".to_owned();
    let mut await_ready = false;

    for m in args {
        match m {
            Meta::NameValue(nv) if nv.path.is_ident("method") => {
                if let Expr::Lit(el) = nv.value {
                    if let Lit::Str(s) = el.lit {
                        method = Some(s.value());
                    } else {
                        return Err(syn::Error::new_spanned(
                            el,
                            "method must be a string literal",
                        ));
                    }
                } else {
                    return Err(syn::Error::new_spanned(
                        nv,
                        "method must be a string literal",
                    ));
                }
            }
            Meta::NameValue(nv) if nv.path.is_ident("stop_timeout") => {
                if let Expr::Lit(el) = nv.value {
                    if let Lit::Str(s) = el.lit {
                        stop_timeout = s.value();
                    } else {
                        return Err(syn::Error::new_spanned(
                            el,
                            "stop_timeout must be a string literal like \"45s\"",
                        ));
                    }
                } else {
                    return Err(syn::Error::new_spanned(
                        nv,
                        "stop_timeout must be a string literal like \"45s\"",
                    ));
                }
            }
            Meta::NameValue(nv) if nv.path.is_ident("await_ready") => {
                if let Expr::Lit(el) = nv.value {
                    if let Lit::Bool(b) = el.lit {
                        await_ready = b.value();
                    } else {
                        return Err(syn::Error::new_spanned(
                            el,
                            "await_ready must be a bool literal (true/false)",
                        ));
                    }
                } else {
                    return Err(syn::Error::new_spanned(
                        nv,
                        "await_ready must be a bool literal (true/false)",
                    ));
                }
            }
            Meta::Path(p) if p.is_ident("await_ready") => {
                await_ready = true;
            }
            other => {
                return Err(syn::Error::new_spanned(
                    other,
                    "expected named args: method=\"...\", stop_timeout=\"...\", await_ready=true|false",
                ));
            }
        }
    }

    let method = method.ok_or_else(|| {
        syn::Error::new(
            Span::call_site(),
            "missing required arg: method=\"runner_name\"",
        )
    })?;
    Ok(LcCfg {
        method,
        stop_timeout,
        await_ready,
    })
}

fn parse_duration_tokens(s: &str) -> syn::Result<proc_macro2::TokenStream> {
    let err = || {
        syn::Error::new(
            Span::call_site(),
            format!("invalid duration: {s}. Use e.g. \"500ms\", \"45s\", \"2m\", \"1h\""),
        )
    };
    if let Some(stripped) = s.strip_suffix("ms") {
        let v: u64 = stripped.parse().map_err(|_| err())?;
        Ok(quote! { ::std::time::Duration::from_millis(#v) })
    } else if let Some(stripped) = s.strip_suffix('s') {
        let v: u64 = stripped.parse().map_err(|_| err())?;
        Ok(quote! { ::std::time::Duration::from_secs(#v) })
    } else if let Some(stripped) = s.strip_suffix('m') {
        let v: u64 = stripped.parse().map_err(|_| err())?;
        Ok(quote! { ::std::time::Duration::from_secs(#v * 60) })
    } else if let Some(stripped) = s.strip_suffix('h') {
        let v: u64 = stripped.parse().map_err(|_| err())?;
        Ok(quote! { ::std::time::Duration::from_secs(#v * 3600) })
    } else {
        Err(err())
    }
}

fn path_last_is(path: &syn::Path, want: &str) -> bool {
    path.segments.last().is_some_and(|s| s.ident == want)
}

// ============================================================================
// Client Generation Macros
// ============================================================================

/// Generate a gRPC client that wraps a tonic-generated service client
///
/// This macro generates a client struct that implements an API trait by delegating
/// to a tonic gRPC client, converting between domain types and protobuf messages.
///
/// # Example
///
/// ```ignore
/// #[modkit::grpc_client(
///     api = "crate::contracts::UsersApi",
///     tonic = "modkit_users_v1::users_service_client::UsersServiceClient<tonic::transport::Channel>",
///     package = "modkit.users.v1"
/// )]
/// pub struct UsersGrpcClient;
/// ```
///
/// This generates:
/// - A struct wrapping the tonic client
/// - An async `connect(uri)` method
/// - A `from_channel(Channel)` constructor
/// - Validation that the client implements the API trait
///
/// Note: The actual trait implementation must be provided manually, as procedural
/// macros cannot introspect trait methods from external modules at compile time.
/// Each method should convert requests/responses using `.into()`.
#[proc_macro_attribute]
pub fn grpc_client(attr: TokenStream, item: TokenStream) -> TokenStream {
    let config = parse_macro_input!(attr as grpc_client::GrpcClientConfig);
    let input = parse_macro_input!(item as DeriveInput);

    match grpc_client::expand_grpc_client(config, input) {
        Ok(expanded) => TokenStream::from(expanded),
        Err(e) => TokenStream::from(e.to_compile_error()),
    }
}

/// Generates API DTO (Data Transfer Object) boilerplate for REST API types.
///
/// This macro automatically derives the necessary traits and attributes for types
/// used in REST API requests and responses, ensuring they follow API conventions.
///
/// # Arguments
///
/// - `request` - Marks the type as a request DTO (adds `Deserialize` and `RequestApiDto`)
/// - `response` - Marks the type as a response DTO (adds `Serialize` and `ResponseApiDto`)
///
/// At least one of `request` or `response` must be specified. Both can be used together
/// for types that serve as both request and response DTOs.
///
/// # Generated Code
///
/// The macro generates:
/// - `#[derive(serde::Serialize)]` if `response` is specified
/// - `#[derive(serde::Deserialize)]` if `request` is specified
/// - `#[derive(utoipa::ToSchema)]` for `OpenAPI` schema generation
/// - `#[serde(rename_all = "snake_case")]` to enforce `snake_case` field naming
/// - `impl RequestApiDto` if `request` is specified
/// - `impl ResponseApiDto` if `response` is specified
///
/// # Examples
///
/// ```ignore
/// // Request-only DTO
/// #[api_dto(request)]
/// pub struct CreateUserRequest {
///     pub user_name: String,
///     pub email: String,
/// }
///
/// // Response-only DTO
/// #[api_dto(response)]
/// pub struct UserResponse {
///     pub id: String,
///     pub user_name: String,
/// }
///
/// // Both request and response
/// #[api_dto(request, response)]
/// pub struct UserDto {
///     pub id: String,
///     pub user_name: String,
/// }
/// ```
///
/// # Field Naming
///
/// All fields are automatically converted to `snake_case` in JSON serialization,
/// regardless of the Rust field name.
#[proc_macro_attribute]
pub fn api_dto(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attrs = parse_macro_input!(attr with Punctuated::<Ident, Token![,]>::parse_terminated);
    let input = parse_macro_input!(item as DeriveInput);
    TokenStream::from(api_dto::expand_api_dto(&attrs, &input))
}

/// Marks a struct or enum as a domain model, enforcing DDD boundaries at compile time.
///
/// This macro:
/// - Implements `DomainModel` for the type
/// - Validates at compile-time that fields do not use forbidden infrastructure types
///
/// # Usage
///
/// ```ignore
/// // Note: This example requires `modkit` crate which is not available in proc-macro doctest context
/// use modkit_macros::domain_model;
///
/// #[domain_model]
/// pub struct User {
///     pub id: i64,
///     pub email: String,
///     pub active: bool,
/// }
/// ```
///
/// # Compile-Time Enforcement
///
/// If any field uses an infrastructure type (e.g., `http::StatusCode`, `sqlx::Pool`),
/// the code will fail to compile with a clear error message:
///
/// ```compile_fail
/// use modkit_macros::domain_model;
///
/// #[domain_model]
/// pub struct BadModel {
///     pub status: http::StatusCode,  // ERROR: forbidden crate 'http'
/// }
/// ```
///
/// # Forbidden Types
///
/// The macro blocks types from infrastructure crates:
/// - Database: `sqlx::*`, `sea_orm::*`
/// - HTTP/Web: `http::*`, `axum::*`, `hyper::*`
/// - External clients: `reqwest::*`, `tonic::*`
/// - File system: `std::fs::*`, `tokio::fs::*`
/// - Database-specific names: `PgPool`, `MySqlPool`, `SqlitePool`, `DatabaseConnection`
#[proc_macro_attribute]
pub fn domain_model(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    TokenStream::from(domain_model::expand_domain_model(&input))
}
