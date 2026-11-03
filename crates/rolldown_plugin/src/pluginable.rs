use std::{any::Any, borrow::Cow, fmt, future::Future, pin::Pin, sync::Arc};

use anyhow::Result;
use rolldown_common::{ModuleInfo, NormalModule, RollupRenderedChunk, WatcherChangeKind};

use super::plugin_context::PluginContext;
pub use crate::plugin::{
  HookAugmentChunkHashReturn, HookHotUpdateReturn, HookLoadReturn, HookNoopReturn,
  HookRenderChunkReturn, HookResolveFileUrlReturn, HookResolveIdReturn, HookTransformAstReturn,
  HookTransformReturn,
};
use crate::{
  HookAddonArgs, HookBuildEndArgs, HookBuildStartArgs, HookCloseBundleArgs, HookGenerateBundleArgs,
  HookInjectionOutputReturn, HookLoadArgs, HookRenderChunkArgs, HookRenderStartArgs,
  HookResolveFileUrlArgs, HookResolveIdArgs, HookTransformArgs, HookUsage, Plugin, PluginHookMeta,
  SharedLoadPluginContext, SharedTransformPluginContext,
  types::{
    hook_hot_update_args::HookHotUpdateArgs, hook_render_error::HookRenderErrorArgs,
    hook_transform_ast_args::HookTransformAstArgs, hook_write_bundle_args::HookWriteBundleArgs,
  },
};

pub type SharedPluginable = Arc<Pluginable>;
type HookFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
type ErasedPlugin = dyn Any + Send + Sync;
type HookMetaFn = fn(&ErasedPlugin) -> Option<PluginHookMeta>;

#[derive(Clone, Copy)]
struct HookFns<F> {
  call: F,
  meta: HookMetaFn,
}

macro_rules! define_hooks {
  (
    $(
      $(#[$attr:meta])*
      $hook:ident: $fn_ty:ident {
        usage: $usage:ident,
        call: $call:ident,
        call_meta: $call_meta:ident,
        meta: $meta:ident,
        default: $default:ident,
        $(adapter_attr: #[$adapter_attr:meta],)?
        args: ($($arg:ident: $arg_ty:ty),* $(,)?),
        output: $output:ty,
      }
    )*
  ) => {
    $(
      type $fn_ty = for<'a> fn(
        &'a ErasedPlugin,
        $($arg_ty),*
      ) -> HookFuture<'a, $output>;
    )*

    struct PluginHooks {
      $($hook: HookFns<$fn_ty>,)*
    }

    impl PluginHooks {
      fn new<T: Plugin>(hook_usage: HookUsage) -> Self {
        Self {
          $($hook: if hook_usage.contains(HookUsage::$usage) {
            HookFns {
              call: $hook::<T> as $fn_ty,
              meta: $call_meta::<T> as HookMetaFn,
            }
          } else {
            HookFns { call: $default as $fn_ty, meta: default_meta }
          },)*
        }
      }
    }

    $(
      $(#[$adapter_attr])?
      fn $hook<'a, T: Plugin>(
        plugin: &'a ErasedPlugin,
        $($arg: $arg_ty),*
      ) -> HookFuture<'a, $output> {
        Box::pin(Plugin::$hook(plugin_ref::<T>(plugin), $($arg),*))
      }

      fn $call_meta<T: Plugin>(plugin: &ErasedPlugin) -> Option<PluginHookMeta> {
        Plugin::$meta(plugin_ref::<T>(plugin))
      }
    )*

    impl Pluginable {
      $(
        $(#[$attr])*
        pub fn $call<'a>(
          &'a self,
          $($arg: $arg_ty),*
        ) -> HookFuture<'a, $output> {
          (self.hooks.$hook.call)(self.plugin.as_ref(), $($arg),*)
        }

        pub fn $call_meta(&self) -> Option<PluginHookMeta> {
          (self.hooks.$hook.meta)(self.plugin.as_ref())
        }
      )*
    }
  };
}

/// The type-erased runtime representation of a [Plugin].
///
/// Hook dispatch is selected once when the plugin is wrapped. Unused hooks point at shared
/// defaults, while implemented hooks point at small type-erasing adapters. This keeps the
/// ergonomic, statically dispatched [Plugin] trait without generating a large object-safe
/// vtable for every plugin type.
pub struct Pluginable {
  plugin: Box<ErasedPlugin>,
  name: fn(&ErasedPlugin) -> Cow<'static, str>,
  hook_usage: HookUsage,
  hooks: PluginHooks,
}

define_hooks! {
  // Build hooks
  build_start: BuildStartFn {
    usage: BuildStart,
    call: call_build_start,
    call_meta: call_build_start_meta,
    meta: build_start_meta,
    default: default_build_start,
    args: (ctx: &'a PluginContext, args: &'a HookBuildStartArgs<'a>),
    output: HookNoopReturn,
  }
  resolve_id: ResolveIdFn {
    usage: ResolveId,
    call: call_resolve_id,
    call_meta: call_resolve_id_meta,
    meta: resolve_id_meta,
    default: default_resolve_id,
    args: (ctx: &'a PluginContext, args: &'a HookResolveIdArgs<'a>),
    output: HookResolveIdReturn,
  }
  #[deprecated(
    note = "This hook is only for rollup compatibility, please use `resolve_id` instead."
  )]
  resolve_dynamic_import: ResolveDynamicImportFn {
    usage: ResolveDynamicImport,
    call: call_resolve_dynamic_import,
    call_meta: call_resolve_dynamic_import_meta,
    meta: resolve_dynamic_import_meta,
    default: default_resolve_id,
    adapter_attr: #[expect(deprecated)],
    args: (ctx: &'a PluginContext, args: &'a HookResolveIdArgs<'a>),
    output: HookResolveIdReturn,
  }
  load: LoadFn {
    usage: Load,
    call: call_load,
    call_meta: call_load_meta,
    meta: load_meta,
    default: default_load,
    args: (ctx: SharedLoadPluginContext, args: &'a HookLoadArgs<'a>),
    output: HookLoadReturn,
  }
  transform: TransformFn {
    usage: Transform,
    call: call_transform,
    call_meta: call_transform_meta,
    meta: transform_meta,
    default: default_transform,
    args: (ctx: SharedTransformPluginContext, args: &'a HookTransformArgs<'a>),
    output: HookTransformReturn,
  }
  transform_ast: TransformAstFn {
    usage: TransformAst,
    call: call_transform_ast,
    call_meta: call_transform_ast_meta,
    meta: transform_ast_meta,
    default: default_transform_ast,
    args: (ctx: &'a PluginContext, args: HookTransformAstArgs<'a>),
    output: HookTransformAstReturn,
  }
  module_parsed: ModuleParsedFn {
    usage: ModuleParsed,
    call: call_module_parsed,
    call_meta: call_module_parsed_meta,
    meta: module_parsed_meta,
    default: default_module_parsed,
    args: (
      ctx: &'a PluginContext,
      module_info: Arc<ModuleInfo>,
      normal_module: &'a NormalModule,
    ),
    output: HookNoopReturn,
  }
  build_end: BuildEndFn {
    usage: BuildEnd,
    call: call_build_end,
    call_meta: call_build_end_meta,
    meta: build_end_meta,
    default: default_build_end,
    args: (ctx: &'a PluginContext, args: Option<&'a HookBuildEndArgs<'a>>),
    output: HookNoopReturn,
  }

  // Generate hooks
  render_start: RenderStartFn {
    usage: RenderStart,
    call: call_render_start,
    call_meta: call_render_start_meta,
    meta: render_start_meta,
    default: default_render_start,
    args: (ctx: &'a PluginContext, args: &'a HookRenderStartArgs<'a>),
    output: HookNoopReturn,
  }
  banner: BannerFn {
    usage: Banner,
    call: call_banner,
    call_meta: call_banner_meta,
    meta: banner_meta,
    default: default_addon,
    args: (ctx: &'a PluginContext, args: &'a HookAddonArgs),
    output: HookInjectionOutputReturn,
  }
  footer: FooterFn {
    usage: Footer,
    call: call_footer,
    call_meta: call_footer_meta,
    meta: footer_meta,
    default: default_addon,
    args: (ctx: &'a PluginContext, args: &'a HookAddonArgs),
    output: HookInjectionOutputReturn,
  }
  intro: IntroFn {
    usage: Intro,
    call: call_intro,
    call_meta: call_intro_meta,
    meta: intro_meta,
    default: default_addon,
    args: (ctx: &'a PluginContext, args: &'a HookAddonArgs),
    output: HookInjectionOutputReturn,
  }
  outro: OutroFn {
    usage: Outro,
    call: call_outro,
    call_meta: call_outro_meta,
    meta: outro_meta,
    default: default_addon,
    args: (ctx: &'a PluginContext, args: &'a HookAddonArgs),
    output: HookInjectionOutputReturn,
  }
  render_chunk: RenderChunkFn {
    usage: RenderChunk,
    call: call_render_chunk,
    call_meta: call_render_chunk_meta,
    meta: render_chunk_meta,
    default: default_render_chunk,
    args: (ctx: &'a PluginContext, args: &'a HookRenderChunkArgs<'a>),
    output: HookRenderChunkReturn,
  }
  augment_chunk_hash: AugmentChunkHashFn {
    usage: AugmentChunkHash,
    call: call_augment_chunk_hash,
    call_meta: call_augment_chunk_hash_meta,
    meta: augment_chunk_hash_meta,
    default: default_augment_chunk_hash,
    args: (ctx: &'a PluginContext, chunk: Arc<RollupRenderedChunk>),
    output: HookAugmentChunkHashReturn,
  }
  resolve_file_url: ResolveFileUrlFn {
    usage: ResolveFileUrl,
    call: call_resolve_file_url,
    call_meta: call_resolve_file_url_meta,
    meta: resolve_file_url_meta,
    default: default_resolve_file_url,
    args: (ctx: &'a PluginContext, args: &'a HookResolveFileUrlArgs<'a>),
    output: HookResolveFileUrlReturn,
  }
  render_error: RenderErrorFn {
    usage: RenderError,
    call: call_render_error,
    call_meta: call_render_error_meta,
    meta: render_error_meta,
    default: default_render_error,
    args: (ctx: &'a PluginContext, args: &'a HookRenderErrorArgs<'a>),
    output: HookNoopReturn,
  }
  generate_bundle: GenerateBundleFn {
    usage: GenerateBundle,
    call: call_generate_bundle,
    call_meta: call_generate_bundle_meta,
    meta: generate_bundle_meta,
    default: default_generate_bundle,
    args: (ctx: &'a PluginContext, args: &'a mut HookGenerateBundleArgs<'a>),
    output: HookNoopReturn,
  }
  write_bundle: WriteBundleFn {
    usage: WriteBundle,
    call: call_write_bundle,
    call_meta: call_write_bundle_meta,
    meta: write_bundle_meta,
    default: default_write_bundle,
    args: (ctx: &'a PluginContext, args: &'a mut HookWriteBundleArgs<'a>),
    output: HookNoopReturn,
  }
  close_bundle: CloseBundleFn {
    usage: CloseBundle,
    call: call_close_bundle,
    call_meta: call_close_bundle_meta,
    meta: close_bundle_meta,
    default: default_close_bundle,
    args: (ctx: &'a PluginContext, args: Option<&'a HookCloseBundleArgs<'a>>),
    output: HookNoopReturn,
  }

  // Watch hooks
  watch_change: WatchChangeFn {
    usage: WatchChange,
    call: call_watch_change,
    call_meta: call_watch_change_meta,
    meta: watch_change_meta,
    default: default_watch_change,
    args: (ctx: &'a PluginContext, path: &'a str, event: WatcherChangeKind),
    output: HookNoopReturn,
  }
  hot_update: HotUpdateFn {
    usage: HotUpdate,
    call: call_hot_update,
    call_meta: call_hot_update_meta,
    meta: hot_update_meta,
    default: default_hot_update,
    args: (ctx: &'a PluginContext, args: &'a HookHotUpdateArgs),
    output: HookHotUpdateReturn,
  }
  close_watcher: CloseWatcherFn {
    usage: CloseWatcher,
    call: call_close_watcher,
    call_meta: call_close_watcher_meta,
    meta: close_watcher_meta,
    default: default_close_watcher,
    args: (ctx: &'a PluginContext),
    output: HookNoopReturn,
  }
}

impl Pluginable {
  #[inline]
  pub fn new<T: Plugin>(plugin: T) -> Self {
    let hook_usage = Plugin::register_hook_usage(&plugin);
    let hooks = PluginHooks::new::<T>(hook_usage);
    Self { plugin: Box::new(plugin), name: name::<T>, hook_usage, hooks }
  }

  #[inline]
  pub fn new_shared<T: Plugin>(plugin: T) -> SharedPluginable {
    Arc::new(Self::new(plugin))
  }

  pub fn call_name(&self) -> Cow<'static, str> {
    (self.name)(self.plugin.as_ref())
  }

  pub fn call_hook_usage(&self) -> HookUsage {
    self.hook_usage
  }
}

impl fmt::Debug for Pluginable {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Plugin").field("name", &self.call_name()).finish_non_exhaustive()
  }
}

#[inline]
fn plugin_ref<T: Plugin>(plugin: &ErasedPlugin) -> &T {
  debug_assert!(plugin.is::<T>());
  // SAFETY: `Pluginable::new` installs only adapters for the same `T` that it stores in
  // `plugin`. The erased trait object's data pointer therefore always points to a valid `T`.
  unsafe { &*std::ptr::from_ref(plugin).cast::<T>() }
}

fn name<T: Plugin>(plugin: &ErasedPlugin) -> Cow<'static, str> {
  Plugin::name(plugin_ref::<T>(plugin))
}

fn default_meta(_plugin: &ErasedPlugin) -> Option<PluginHookMeta> {
  None
}

fn default_noop<'a>() -> HookFuture<'a, HookNoopReturn> {
  Box::pin(async { Ok(()) })
}

fn default_optional<'a, T: Send + 'a>() -> HookFuture<'a, Result<Option<T>>> {
  Box::pin(async { Ok(None) })
}

macro_rules! default_hook {
  (
    $adapter:ident(
      $($arg:ident: $arg_ty:ty),* $(,)?
    ) -> $output:ty = $body:expr
  ) => {
    fn $adapter<'a>(
      _plugin: &'a ErasedPlugin,
      $($arg: $arg_ty),*
    ) -> HookFuture<'a, $output> {
      $(let _ = $arg;)*
      $body
    }
  };
}

default_hook!(
  default_build_start(
    ctx: &'a PluginContext,
    args: &'a HookBuildStartArgs<'a>
  ) -> HookNoopReturn = default_noop()
);
default_hook!(
  default_resolve_id(
    ctx: &'a PluginContext,
    args: &'a HookResolveIdArgs<'a>
  ) -> HookResolveIdReturn = default_optional()
);
default_hook!(
  default_load(
    ctx: SharedLoadPluginContext,
    args: &'a HookLoadArgs<'a>
  ) -> HookLoadReturn = default_optional()
);
default_hook!(
  default_transform(
    ctx: SharedTransformPluginContext,
    args: &'a HookTransformArgs<'a>
  ) -> HookTransformReturn = default_optional()
);
fn default_transform_ast<'a>(
  _plugin: &'a ErasedPlugin,
  _ctx: &'a PluginContext,
  args: HookTransformAstArgs<'a>,
) -> HookFuture<'a, HookTransformAstReturn> {
  Box::pin(async { Ok(args.ast) })
}
default_hook!(
  default_module_parsed(
    ctx: &'a PluginContext,
    module_info: Arc<ModuleInfo>,
    normal_module: &'a NormalModule
  ) -> HookNoopReturn = default_noop()
);
default_hook!(
  default_build_end(
    ctx: &'a PluginContext,
    args: Option<&'a HookBuildEndArgs<'a>>
  ) -> HookNoopReturn = default_noop()
);
default_hook!(
  default_render_start(
    ctx: &'a PluginContext,
    args: &'a HookRenderStartArgs<'a>
  ) -> HookNoopReturn = default_noop()
);
default_hook!(
  default_addon(
    ctx: &'a PluginContext,
    args: &'a HookAddonArgs
  ) -> HookInjectionOutputReturn = default_optional()
);
default_hook!(
  default_render_chunk(
    ctx: &'a PluginContext,
    args: &'a HookRenderChunkArgs<'a>
  ) -> HookRenderChunkReturn = default_optional()
);
default_hook!(
  default_augment_chunk_hash(
    ctx: &'a PluginContext,
    chunk: Arc<RollupRenderedChunk>
  ) -> HookAugmentChunkHashReturn = default_optional()
);
default_hook!(
  default_resolve_file_url(
    ctx: &'a PluginContext,
    args: &'a HookResolveFileUrlArgs<'a>
  ) -> HookResolveFileUrlReturn = default_optional()
);
default_hook!(
  default_render_error(
    ctx: &'a PluginContext,
    args: &'a HookRenderErrorArgs<'a>
  ) -> HookNoopReturn = default_noop()
);
default_hook!(
  default_generate_bundle(
    ctx: &'a PluginContext,
    args: &'a mut HookGenerateBundleArgs<'a>
  ) -> HookNoopReturn = default_noop()
);
default_hook!(
  default_write_bundle(
    ctx: &'a PluginContext,
    args: &'a mut HookWriteBundleArgs<'a>
  ) -> HookNoopReturn = default_noop()
);
default_hook!(
  default_close_bundle(
    ctx: &'a PluginContext,
    args: Option<&'a HookCloseBundleArgs<'a>>
  ) -> HookNoopReturn = default_noop()
);
default_hook!(
  default_watch_change(
    ctx: &'a PluginContext,
    path: &'a str,
    event: WatcherChangeKind
  ) -> HookNoopReturn = default_noop()
);
default_hook!(
  default_hot_update(
    ctx: &'a PluginContext,
    args: &'a HookHotUpdateArgs
  ) -> HookHotUpdateReturn = default_optional()
);
default_hook!(
  default_close_watcher(
    ctx: &'a PluginContext
  ) -> HookNoopReturn = default_noop()
);
