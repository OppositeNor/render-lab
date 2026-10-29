//! `#[derive(Values)]`: a struct whose fields a timeline, a story, and the
//! dock reach by key (the lab's src/values.rs, `lab::values::Reflect`).
//!
//! ```ignore
//! #[derive(Clone, lab::Values)]
//! #[values(scope = "camera", package = "rlstd")]
//! pub struct OrbitCamera {
//!     #[value(components(x, y, z))]
//!     pub target: [f32; 3],                  // camera.target; camera.x, camera.y, camera.z
//!     /// Degrees around +Y.
//!     #[value(range = -180.0..=180.0, unit = deg, wrap, label(view))]
//!     pub yaw: f32,                          // camera.yaw
//! }
//! ```
//!
//! On the struct: `scope`, the prefix of its keys (`camera.yaw`; none
//! without it), `package`, whose text `text` keys name, and
//! `rename_all = "camelCase"` (a field `light_intensity` is the key
//! `lightIntensity`). On a field: `key` (its key in the scope; the field's
//! name by default), `default`, `range = a..=b`, `step`, `unit` (`m`, `deg`,
//! `factor`, `plain`, `index`), `wrap` (an angle shown wrapped into its
//! range), `label(a, b, …)`, `text` (a key of the package's text, its name
//! on screen), `name` (its name on screen in every language), `color` (a
//! constant `egui::Color32`, its name's colour in the dock), `toggle` (a
//! number used as on or off), and, for a vector, `components(x, y, z)`, its
//! components' keys in the scope (`key.x`, … by default). Its `///` comment
//! is its `doc`. A field without `#[value]` is a value too, with no
//! metadata. The field types are `f32`, `i32`, `u32`, `bool`, and
//! `[f32; 2]`, `[f32; 3]`, `[f32; 4]`. When every field has a `default`, the
//! struct also gets `fn defaults() -> Self`.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as Tokens;
use quote::quote;
use syn::spanned::Spanned;
use syn::{parse_macro_input, Data, DeriveInput, Error, Expr, Fields, LitStr, Result};

/// `#[derive(Choice)]` on an enum of unit variants: its variants are a
/// choice's options, in order (the lab's `values::Choice`).
///
/// ```ignore
/// #[derive(Clone, Copy, lab::Choice)]
/// #[choice(package = "o3de_lighting", wgsl = "VIEW")]
/// pub enum ViewMode {
///     #[choice(text = "controls.final")]
///     Final,                       // "final", VIEW_FINAL = 0u
///     #[choice(label = "D (GGX)")]
///     D,                           // "d", VIEW_D = 1u
/// }
/// ```
///
/// On the enum: `package` (whose text `text` keys name) and `wgsl` (its
/// constants' prefix, when a shader uses them). On a variant: `name` (its
/// name in stories; the variant's in camelCase by default), `text` (a key of
/// the package's text, its name on screen), `label` (its name on screen in
/// every language), and `wgsl` (its constant's name after the prefix; the
/// variant's in SCREAMING_SNAKE_CASE by default). A typed view reads a choice
/// by its option's name through serde, so an enum that derives serde's traits
/// names its variants the same way: `#[serde(rename_all = "camelCase")]`, and
/// `#[serde(rename = "…")]` beside a variant's `name`.
#[proc_macro_derive(Choice, attributes(choice))]
pub fn derive_choice(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_choice(&input).unwrap_or_else(Error::into_compile_error).into()
}

fn expand_choice(input: &DeriveInput) -> Result<Tokens> {
    let name = &input.ident;
    let Data::Enum(data) = &input.data else { return Err(Error::new(input.span(), "`Choice` is derived for an enum")) };
    let (mut package, mut prefix) = (String::new(), None::<String>);
    for attr in input.attrs.iter().filter(|a| a.path().is_ident("choice")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("package") {
                package = meta.value()?.parse::<LitStr>()?.value();
            } else if meta.path.is_ident("wgsl") {
                prefix = Some(meta.value()?.parse::<LitStr>()?.value());
            } else {
                return Err(meta.error("unknown `#[choice]` attribute of an enum (package, wgsl)"));
            }
            Ok(())
        })?;
    }
    let (mut options, mut indices, mut variants) = (Vec::new(), Vec::new(), Vec::new());
    for (i, variant) in data.variants.iter().enumerate() {
        if !matches!(variant.fields, Fields::Unit) {
            return Err(Error::new(variant.span(), "a choice's option is a variant without fields"));
        }
        let ident = &variant.ident;
        let text_name = ident.to_string();
        let mut story = {
            let mut c = text_name.chars();
            c.next().map(|f| f.to_lowercase().collect::<String>() + c.as_str()).unwrap_or_default()
        };
        let mut wgsl = String::new();
        for (j, c) in text_name.chars().enumerate() {
            if c.is_uppercase() && j > 0 {
                wgsl.push('_');
            }
            wgsl.extend(c.to_uppercase());
        }
        let (mut text, mut label) = (None::<String>, None::<String>);
        for attr in variant.attrs.iter().filter(|a| a.path().is_ident("choice")) {
            attr.parse_nested_meta(|meta| {
                let value = meta.value()?.parse::<LitStr>()?.value();
                if meta.path.is_ident("name") {
                    story = value;
                } else if meta.path.is_ident("text") {
                    text = Some(value);
                } else if meta.path.is_ident("label") {
                    label = Some(value);
                } else if meta.path.is_ident("wgsl") {
                    wgsl = value;
                } else {
                    return Err(meta.error("unknown `#[choice]` attribute of a variant (name, text, label, wgsl)"));
                }
                Ok(())
            })?;
        }
        if text.is_some() && package.is_empty() {
            return Err(Error::new(variant.span(), "`text` names a key of a package's text: give the enum `#[choice(package = \"...\")]`"));
        }
        let text = match text {
            Some(t) => quote!(Some(#t)),
            None => quote!(None),
        };
        let label = match label {
            Some(l) => quote!(Some(#l)),
            None => quote!(None),
        };
        let i = i as u32;
        options.push(quote!(::lab::values::ChoiceOption { name: #story, text: #text, label: #label, wgsl: #wgsl }));
        indices.push(quote!(#name::#ident => #i));
        variants.push(quote!(#i => Some(#name::#ident)));
    }
    let enum_name = name.to_string();
    let prefix = match prefix {
        Some(p) => quote!(Some(#p)),
        None => quote!(None),
    };
    let count = options.len();
    Ok(quote! {
        impl ::lab::values::Choice for #name {
            fn choices() -> &'static ::lab::values::Choices {
                static OPTIONS: [::lab::values::ChoiceOption; #count] = [#(#options),*];
                static CHOICES: ::lab::values::Choices = ::lab::values::Choices { name: #enum_name, package: #package, wgsl: #prefix, options: &OPTIONS };
                &CHOICES
            }
            fn index(self) -> u32 {
                match self {
                    #(#indices,)*
                }
            }
            fn from_index(index: u32) -> Option<Self> {
                match index {
                    #(#variants,)*
                    _ => None,
                }
            }
        }
    })
}

#[proc_macro_derive(Values, attributes(values, value))]
pub fn derive_values(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand(&input).unwrap_or_else(Error::into_compile_error).into()
}

/// A field's type, as `lab::values::Type` names it.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    F32,
    I32,
    U32,
    Bool,
    Vec(usize),
}

impl Kind {
    fn of(ty: &syn::Type) -> Result<Kind> {
        let text = quote!(#ty).to_string().replace(' ', "");
        Ok(match text.as_str() {
            "f32" => Kind::F32,
            "i32" => Kind::I32,
            "u32" => Kind::U32,
            "bool" => Kind::Bool,
            "[f32;2]" => Kind::Vec(2),
            "[f32;3]" => Kind::Vec(3),
            "[f32;4]" => Kind::Vec(4),
            _ => return Err(Error::new(ty.span(), "a value is f32, i32, u32, bool, or [f32; 2], [f32; 3], [f32; 4]")),
        })
    }

    fn ty(self) -> Tokens {
        match self {
            Kind::F32 => quote!(::lab::values::Type::F32),
            Kind::I32 => quote!(::lab::values::Type::I32),
            Kind::U32 => quote!(::lab::values::Type::U32),
            Kind::Bool => quote!(::lab::values::Type::Bool),
            Kind::Vec(2) => quote!(::lab::values::Type::Vec2),
            Kind::Vec(3) => quote!(::lab::values::Type::Vec3),
            Kind::Vec(_) => quote!(::lab::values::Type::Vec4),
        }
    }

    /// `value` (a const expression) as a `Value` of this kind.
    fn value(self, value: &Expr) -> Tokens {
        match self {
            Kind::F32 => quote!(::lab::values::Value::F32(#value as f32)),
            Kind::I32 => quote!(::lab::values::Value::I32(#value as i32)),
            Kind::U32 => quote!(::lab::values::Value::U32(#value as u32)),
            Kind::Bool => quote!(::lab::values::Value::Bool(#value)),
            Kind::Vec(2) => quote!(::lab::values::Value::Vec2(#value)),
            Kind::Vec(3) => quote!(::lab::values::Value::Vec3(#value)),
            Kind::Vec(_) => quote!(::lab::values::Value::Vec4(#value)),
        }
    }
}

/// What `#[value(..)]` says of a field.
#[derive(Default)]
struct FieldAttrs {
    key: Option<String>,
    default: Option<Expr>,
    range: Option<(Expr, Expr)>,
    step: Option<Expr>,
    unit: Option<syn::Ident>,
    wrap: bool,
    labels: Vec<String>,
    text: Option<String>,
    name: Option<String>,
    color: Option<Expr>,
    toggle: bool,
    components: Vec<String>,
}

fn field_attrs(field: &syn::Field) -> Result<FieldAttrs> {
    let mut out = FieldAttrs::default();
    for attr in field.attrs.iter().filter(|a| a.path().is_ident("value")) {
        attr.parse_nested_meta(|meta| {
            let name = meta.path.get_ident().map(|i| i.to_string()).unwrap_or_default();
            match name.as_str() {
                "key" => out.key = Some(meta.value()?.parse::<LitStr>()?.value()),
                "default" => out.default = Some(meta.value()?.parse()?),
                "step" => out.step = Some(meta.value()?.parse()?),
                "text" => out.text = Some(meta.value()?.parse::<LitStr>()?.value()),
                "name" => out.name = Some(meta.value()?.parse::<LitStr>()?.value()),
                "color" => out.color = Some(meta.value()?.parse()?),
                "toggle" => out.toggle = true,
                "unit" => {
                    let unit: syn::Ident = meta.value()?.parse()?;
                    if !["m", "deg", "factor", "plain", "index"].contains(&unit.to_string().as_str()) {
                        return Err(Error::new(unit.span(), "a unit is m, deg, factor, plain, or index"));
                    }
                    out.unit = Some(unit);
                }
                "range" => {
                    let range: syn::ExprRange = meta.value()?.parse()?;
                    match (range.start, range.limits, range.end) {
                        (Some(start), syn::RangeLimits::Closed(_), Some(end)) => out.range = Some((*start, *end)),
                        _ => return Err(meta.error("a range is `low..=high`")),
                    }
                }
                "wrap" => out.wrap = true,
                "label" => meta.parse_nested_meta(|label| {
                    out.labels.push(label.path.get_ident().ok_or_else(|| label.error("a label is a name"))?.to_string());
                    Ok(())
                })?,
                "components" => meta.parse_nested_meta(|c| {
                    out.components.push(c.path.get_ident().ok_or_else(|| c.error("a component is a name"))?.to_string());
                    Ok(())
                })?,
                _ => return Err(meta.error("unknown `#[value]` attribute (key, default, range, step, unit, wrap, label, text, name, color, toggle, components)")),
            }
            Ok(())
        })?;
    }
    Ok(out)
}

/// The text of a field's `///` comment, its lines joined.
fn doc(attrs: &[syn::Attribute]) -> String {
    let lines: Vec<String> = attrs
        .iter()
        .filter(|a| a.path().is_ident("doc"))
        .filter_map(|a| match &a.meta {
            syn::Meta::NameValue(nv) => match &nv.value {
                Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) => Some(s.value().trim().to_string()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    lines.join(" ")
}

fn expand(input: &DeriveInput) -> Result<Tokens> {
    let name = &input.ident;
    let Data::Struct(data) = &input.data else { return Err(Error::new(input.span(), "`Values` is derived for a struct")) };
    let Fields::Named(fields) = &data.fields else { return Err(Error::new(input.span(), "`Values` needs named fields")) };
    let (mut scope, mut package, mut camel) = (String::new(), String::new(), false);
    for attr in input.attrs.iter().filter(|a| a.path().is_ident("values")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("scope") {
                scope = meta.value()?.parse::<LitStr>()?.value();
            } else if meta.path.is_ident("package") {
                package = meta.value()?.parse::<LitStr>()?.value();
            } else if meta.path.is_ident("rename_all") {
                let rule = meta.value()?.parse::<LitStr>()?;
                if rule.value() != "camelCase" {
                    return Err(Error::new(rule.span(), "`rename_all` is \"camelCase\""));
                }
                camel = true;
            } else {
                return Err(meta.error("unknown `#[values]` attribute (scope, package, rename_all)"));
            }
            Ok(())
        })?;
    }
    let scoped = |key: &str| if scope.is_empty() { key.to_string() } else { format!("{scope}.{key}") };
    let camel_case = |name: &str| {
        let mut out = String::new();
        let mut upper = false;
        for c in name.chars() {
            if c == '_' {
                upper = true;
            } else if upper {
                out.extend(c.to_uppercase());
                upper = false;
            } else {
                out.push(c);
            }
        }
        out
    };
    let (mut entries, mut gets, mut sets, mut defaults) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut all_defaults = true;
    for field in &fields.named {
        let ident = field.ident.as_ref().unwrap();
        let kind = Kind::of(&field.ty)?;
        let attrs = field_attrs(field)?;
        if attrs.text.is_some() && package.is_empty() {
            return Err(Error::new(field.span(), "`text` names a key of a package's text: give the struct `#[values(package = \"...\")]`"));
        }
        let field_name = if camel { camel_case(&ident.to_string()) } else { ident.to_string() };
        let key = scoped(attrs.key.as_deref().unwrap_or(&field_name));
        let components: Vec<String> = match kind {
            Kind::Vec(n) => {
                if !attrs.components.is_empty() && attrs.components.len() != n {
                    return Err(Error::new(field.span(), format!("a [f32; {n}] has {n} components")));
                }
                let names: Vec<String> = if attrs.components.is_empty() { ["x", "y", "z", "w"][..n].iter().map(|c| format!("{key}.{c}")).collect() } else { attrs.components.iter().map(|c| scoped(c)).collect() };
                names
            }
            _ if !attrs.components.is_empty() => return Err(Error::new(field.span(), "only a vector has components")),
            _ => Vec::new(),
        };
        let ty = kind.ty();
        let default = match &attrs.default {
            Some(v) => {
                defaults.push(match kind {
                    Kind::F32 => quote!(#ident: (#v) as f32),
                    Kind::I32 => quote!(#ident: (#v) as i32),
                    Kind::U32 => quote!(#ident: (#v) as u32),
                    _ => quote!(#ident: #v),
                });
                let v = kind.value(v);
                quote!(Some(#v))
            }
            None => {
                all_defaults = false;
                quote!(None)
            }
        };
        let range = match &attrs.range {
            Some((a, b)) => quote!(Some((#a as f32, #b as f32))),
            None => quote!(None),
        };
        let step = match &attrs.step {
            Some(s) => quote!(Some(#s as f32)),
            None => quote!(None),
        };
        let unit = match attrs.unit.as_ref().map(|u| u.to_string()).as_deref() {
            Some("m") => quote!(::lab::values::Unit::Metres),
            Some("deg") => quote!(::lab::values::Unit::Degrees),
            Some("factor") => quote!(::lab::values::Unit::Factor),
            Some("index") => quote!(::lab::values::Unit::Index),
            _ => quote!(::lab::values::Unit::Plain),
        };
        let wrap = attrs.wrap;
        let labels = &attrs.labels;
        let text = match &attrs.text {
            Some(t) => quote!(Some(#t)),
            None => quote!(None),
        };
        let doc = doc(&field.attrs);
        let name = match &attrs.name {
            Some(n) => quote!(Some(#n)),
            None => quote!(None),
        };
        let color = match &attrs.color {
            Some(c) => quote!(Some(#c)),
            None => quote!(None),
        };
        let toggle = attrs.toggle;
        entries.push(quote! {
            ::lab::values::Entry { key: #key, ty: #ty, default: #default, range: #range, component_ranges: &[], step: #step, unit: #unit, wrap: #wrap,
                labels: &[#(#labels),*], text: #text, name: #name, color: #color, toggle: #toggle, choices: None, package: #package, doc: #doc, components: &[#(#components),*] }
        });
        match kind {
            Kind::Vec(n) => {
                let variant = syn::Ident::new(&format!("Vec{n}"), ident.span());
                gets.push(quote!(#key => Some(::lab::values::Value::#variant(self.#ident))));
                sets.push(quote!(#key => match value { ::lab::values::Value::#variant(v) => self.#ident = v, _ => panic!("{} is a {}, not a {:?}", #key, stringify!(#variant), value) }));
                for (i, c) in components.iter().enumerate() {
                    gets.push(quote!(#c => Some(::lab::values::Value::F32(self.#ident[#i]))));
                    sets.push(quote!(#c => self.#ident[#i] = value.f32()));
                }
            }
            _ => {
                let read = match kind {
                    Kind::F32 => quote!(::lab::values::Value::F32(self.#ident)),
                    Kind::I32 => quote!(::lab::values::Value::I32(self.#ident)),
                    Kind::U32 => quote!(::lab::values::Value::U32(self.#ident)),
                    _ => quote!(::lab::values::Value::Bool(self.#ident)),
                };
                let write = match kind {
                    Kind::F32 => quote!(value.f32()),
                    Kind::I32 => quote!(value.i32()),
                    Kind::U32 => quote!(value.u32()),
                    _ => quote!(value.bool()),
                };
                gets.push(quote!(#key => Some(#read)));
                sets.push(quote!(#key => self.#ident = #write));
            }
        }
    }
    let count = entries.len();
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let constructor = if all_defaults {
        quote! {
            impl #impl_generics #name #ty_generics #where_clause {
                /// Every value at its declared default.
                pub fn defaults() -> Self {
                    #name { #(#defaults),* }
                }
            }
        }
    } else {
        quote!()
    };
    Ok(quote! {
        #constructor
        impl #impl_generics ::lab::values::Reflect for #name #ty_generics #where_clause {
            fn schema() -> &'static [::lab::values::Entry] {
                static SCHEMA: [::lab::values::Entry; #count] = [#(#entries),*];
                &SCHEMA
            }
            fn get(&self, key: &str) -> Option<::lab::values::Value> {
                match key {
                    #(#gets,)*
                    _ => None,
                }
            }
            fn set(&mut self, key: &str, value: ::lab::values::Value) -> bool {
                match key {
                    #(#sets,)*
                    _ => return false,
                }
                true
            }
        }
    })
}

/// `#[lab::actions]` on an `impl` block: its `#[action]` functions are the
/// package's actions, which a story starts (`{@ run=key run.arg=…}`) and stops
/// (the lab's src/actions.rs). It gives the type `ACTIONS: lab::actions::Actions`.
///
/// ```ignore
/// #[lab::actions]
/// impl Sdfgi {
///     /// Steps the jump flood, one pass every `seconds`.
///     #[action]
///     fn jump_flood(frame: &mut ActionFrame, seconds: f32) { … }
/// }
/// ```
///
/// An action is a function, not a method: its first parameter is the
/// `&mut ActionFrame` it runs in, and the rest are its arguments, each a type
/// that implements `lab::actions::ActionArg` (their kinds come from those
/// types). It returns nothing, or a `Result` whose error is shown. Its key is
/// its name in camelCase, unless `#[action(key = "…")]` names another; an
/// argument's key is its parameter's name in camelCase; its `///` comment is
/// its doc. The block's other items are left as they are.
#[proc_macro_attribute]
pub fn actions(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        return Error::new(Tokens::from(attr).span(), "`#[lab::actions]` takes no arguments").into_compile_error().into();
    }
    let input = parse_macro_input!(item as syn::ItemImpl);
    expand_actions(input).unwrap_or_else(Error::into_compile_error).into()
}

fn camel(name: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for c in name.trim_start_matches('_').chars() {
        if c == '_' {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

fn expand_actions(mut input: syn::ItemImpl) -> Result<Tokens> {
    if input.trait_.is_some() {
        return Err(Error::new(input.span(), "`#[lab::actions]` goes on a type's own `impl` block, not a trait's"));
    }
    let ty = input.self_ty.clone();
    let (mut specs, mut arms) = (Vec::new(), Vec::new());
    let mut keys: Vec<String> = Vec::new();
    for item in &mut input.items {
        let syn::ImplItem::Fn(f) = item else { continue };
        let Some(at) = f.attrs.iter().position(|a| a.path().is_ident("action")) else { continue };
        let attr = f.attrs.remove(at);
        let ident = f.sig.ident.clone();
        let mut key = camel(&ident.to_string());
        if !matches!(attr.meta, syn::Meta::Path(_)) {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("key") {
                    key = meta.value()?.parse::<LitStr>()?.value();
                    Ok(())
                } else {
                    Err(meta.error("unknown `#[action]` attribute (key)"))
                }
            })?;
        }
        if keys.contains(&key) {
            return Err(Error::new(ident.span(), format!("two actions have the key `{key}`")));
        }
        keys.push(key.clone());
        let doc = doc(&f.attrs);
        let mut inputs = f.sig.inputs.iter();
        match inputs.next() {
            Some(syn::FnArg::Typed(_)) => {}
            Some(syn::FnArg::Receiver(r)) => return Err(Error::new(r.span(), "an action is a function of its frame and arguments, without `self`")),
            None => return Err(Error::new(f.sig.span(), "an action's first parameter is its `&mut ActionFrame`")),
        }
        let (mut args, mut reads) = (Vec::new(), Vec::new());
        for input in inputs {
            let syn::FnArg::Typed(pt) = input else { unreachable!("only the first parameter can be `self`") };
            let syn::Pat::Ident(name) = &*pt.pat else { return Err(Error::new(pt.pat.span(), "an action's argument is a plain name")) };
            let arg_key = camel(&name.ident.to_string());
            let arg_ty = &pt.ty;
            args.push(quote! {
                ::lab::actions::ArgSpec {
                    key: #arg_key,
                    kind: <#arg_ty as ::lab::actions::ActionArg>::KIND,
                    required: <#arg_ty as ::lab::actions::ActionArg>::REQUIRED,
                    check: <#arg_ty as ::lab::actions::ActionArg>::check,
                }
            });
            reads.push(quote! { action.arg::<#arg_ty>(#arg_key)? });
        }
        specs.push(quote! { ::lab::actions::ActionSpec { key: #key, doc: #doc, args: &[#(#args),*] } });
        let call = quote! { <#ty>::#ident(frame, #(#reads),*) };
        arms.push(match f.sig.output {
            syn::ReturnType::Default => quote! { #key => { #call; Ok(()) } },
            _ => quote! { #key => #call.map_err(|e| format!("the action `{}`: {e}", #key)) },
        });
    }
    if specs.is_empty() {
        return Err(Error::new(input.span(), "`#[lab::actions]` finds no `#[action]` function in the block"));
    }
    Ok(quote! {
        #input

        impl #ty {
            /// The actions of this block's `#[action]` functions (`Investigation::actions`).
            pub const ACTIONS: ::lab::actions::Actions = ::lab::actions::Actions { specs: &[#(#specs),*], run: <#ty>::__lab_run_action };

            #[doc(hidden)]
            fn __lab_run_action(action: &::lab::actions::RunningAction, frame: &mut ::lab::actions::ActionFrame) -> ::std::result::Result<(), ::std::string::String> {
                match action.key.as_str() {
                    #(#arms,)*
                    other => Err(format!("no action `{other}`")),
                }
            }
        }
    })
}
