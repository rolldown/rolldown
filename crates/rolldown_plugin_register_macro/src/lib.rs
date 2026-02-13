use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ImplItem, ItemImpl};

/// Automatically generates the `register_hook_usage` method for Plugin implementations.
///
/// This macro analyzes which hook methods are implemented in your Plugin impl block
/// and automatically generates the `register_hook_usage` method that returns the
/// appropriate `HookUsage` flags.
///
/// # Example
///
/// ```rust,ignore
/// use rolldown_plugin::{Plugin, HookUsage};
/// use rolldown_plugin_register_macro::RegisterHook;
///
/// struct MyPlugin;
///
/// #[RegisterHook]
/// impl Plugin for MyPlugin {
///     fn name(&self) -> Cow<'static, str> {
///         Cow::Borrowed("my-plugin")
///     }
///
///     async fn build_start(&self, ctx: &PluginContext, args: &HookBuildStartArgs) -> HookNoopReturn {
///         // implementation
///         Ok(())
///     }
///
///     async fn transform(&self, ctx: SharedTransformPluginContext, args: &HookTransformArgs) -> HookTransformReturn {
///         // implementation
///         Ok(None)
///     }
/// }
/// ```
///
/// The macro will automatically generate:
///
/// ```rust,ignore
/// fn register_hook_usage(&self) -> HookUsage {
///     HookUsage::BuildStart | HookUsage::Transform
/// }
/// ```
#[proc_macro_attribute]
#[expect(non_snake_case, reason = "RegisterHook is a macro name, PascalCase is conventional")]
pub fn RegisterHook(_attr: TokenStream, item: TokenStream) -> TokenStream {
  let input = parse_macro_input!(item as ItemImpl);

  // Map of hook method names to their corresponding HookUsage variant names
  let hook_map = vec![
    ("build_start", "BuildStart"),
    ("resolve_id", "ResolveId"),
    ("resolve_dynamic_import", "ResolveDynamicImport"),
    ("load", "Load"),
    ("transform", "Transform"),
    ("module_parsed", "ModuleParsed"),
    ("build_end", "BuildEnd"),
    ("render_start", "RenderStart"),
    ("render_error", "RenderError"),
    ("render_chunk", "RenderChunk"),
    ("augment_chunk_hash", "AugmentChunkHash"),
    ("generate_bundle", "GenerateBundle"),
    ("write_bundle", "WriteBundle"),
    ("close_bundle", "CloseBundle"),
    ("watch_change", "WatchChange"),
    ("close_watcher", "CloseWatcher"),
    ("transform_ast", "TransformAst"),
    ("banner", "Banner"),
    ("footer", "Footer"),
    ("intro", "Intro"),
    ("outro", "Outro"),
  ];

  // Collect implemented hooks
  let mut implemented_hooks = Vec::new();

  for item in &input.items {
    if let ImplItem::Fn(method) = item {
      let method_name = method.sig.ident.to_string();

      // Find matching hook
      if let Some((_, hook_variant)) =
        hook_map.iter().find(|(hook_name, _)| *hook_name == method_name)
      {
        implemented_hooks.push(hook_variant);
      }
    }
  }

  // Generate the register_hook_usage method
  let register_method = if implemented_hooks.is_empty() {
    // If no hooks are implemented, return empty HookUsage
    quote! {
      fn register_hook_usage(&self) -> rolldown_plugin::HookUsage {
        rolldown_plugin::HookUsage::empty()
      }
    }
  } else if implemented_hooks.len() == 1 {
    // Single hook
    let hook = syn::Ident::new(implemented_hooks[0], proc_macro2::Span::call_site());
    quote! {
      fn register_hook_usage(&self) -> rolldown_plugin::HookUsage {
        rolldown_plugin::HookUsage::#hook
      }
    }
  } else {
    // Multiple hooks - use | operator
    let hook_idents: Vec<_> = implemented_hooks
      .iter()
      .map(|hook| syn::Ident::new(hook, proc_macro2::Span::call_site()))
      .collect();

    quote! {
      fn register_hook_usage(&self) -> rolldown_plugin::HookUsage {
        #(rolldown_plugin::HookUsage::#hook_idents)|*
      }
    }
  };

  // Create a new impl block with the additional method
  let self_ty = &input.self_ty;
  let trait_path = &input.trait_;
  let generics = &input.generics;
  let where_clause = &input.generics.where_clause;
  let existing_items = &input.items;

  let expanded = if let Some((bang, path, _for)) = trait_path {
    quote! {
      impl #generics #bang #path for #self_ty #where_clause {
        #(#existing_items)*

        #register_method
      }
    }
  } else {
    quote! {
      impl #generics #self_ty #where_clause {
        #(#existing_items)*

        #register_method
      }
    }
  };

  TokenStream::from(expanded)
}
