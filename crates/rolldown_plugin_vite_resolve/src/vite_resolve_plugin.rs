use std::{
  borrow::Cow,
  env,
  future::Future,
  path::{Path, PathBuf},
  pin::Pin,
  sync::Arc,
};

use crate::{
  CallablePlugin, ResolveOptionsExternal,
  external::{self, ExternalDecider, ExternalDeciderOptions},
  file_url::file_url_str_to_path,
  resolver::{self, AdditionalOptions, Resolvers},
  utils::{
    BROWSER_EXTERNAL_ID, OPTIONAL_PEER_DEP_ID, clean_url, is_bare_import, is_builtin,
    is_in_node_modules, is_windows_drive_path, normalize_path,
  },
};
use anyhow::anyhow;
use derive_more::Debug;
use rolldown_common::{ImportKind, WatcherChangeKind, side_effects::HookSideEffects};
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookNoopReturn, HookResolveIdArgs,
  HookResolveIdOutput, HookResolveIdReturn, HookUsage, Plugin, PluginContext,
  typedmap::TypedMapKey,
};
use rustc_hash::FxHashSet;
use sugar_path::SugarPath;

const FS_PREFIX: &str = "/@fs/";

#[derive(Debug)]
pub struct ViteResolveOptions {
  pub resolve_options: ViteResolveResolveOptions,
  pub environment_consumer: String,
  pub environment_name: String,
  pub external: external::ResolveOptionsExternal,
  pub no_external: external::ResolveOptionsNoExternal,
  pub dedupe: Vec<String>,
  #[debug(skip)]
  pub finalize_bare_specifier: Option<Arc<FinalizeBareSpecifierCallback>>,
  #[debug(skip)]
  pub finalize_other_specifiers: Option<Arc<FinalizeOtherSpecifiersCallback>>,

  pub runtime: String,
}
pub type FinalizeBareSpecifierCallback = dyn (Fn(
    &str,
    &str,
    Option<&str>,
  ) -> Pin<Box<(dyn Future<Output = anyhow::Result<Option<String>>> + Send + Sync)>>)
  + Send
  + Sync;

pub type FinalizeOtherSpecifiersCallback = dyn (Fn(&str, &str) -> Pin<Box<(dyn Future<Output = anyhow::Result<Option<String>>> + Send + Sync)>>)
  + Send
  + Sync;

#[derive(Debug)]
pub struct ViteResolveResolveOptions {
  pub is_build: bool,
  pub is_production: bool,
  pub as_src: bool,
  pub prefer_relative: bool,
  pub is_require: Option<bool>,
  pub root: String,
  pub scan: bool,

  pub main_fields: Vec<String>,
  pub conditions: Vec<String>,
  pub external_conditions: Vec<String>,
  pub extensions: Vec<String>,
  pub try_index: bool,
  pub try_prefix: Option<String>,
  pub preserve_symlinks: bool,
}

#[derive(Hash, PartialEq, Eq)]
pub struct ResolveIdOptionsScan;

impl TypedMapKey for ResolveIdOptionsScan {
  type Value = bool;
}

#[derive(Debug)]
pub struct ViteResolvePlugin {
  resolve_options: ViteResolveResolveOptions,
  external: external::ResolveOptionsExternal,
  no_external: Arc<external::ResolveOptionsNoExternal>,
  dedupe: Arc<FxHashSet<String>>,
  environment_consumer: String,
  environment_name: String,
  #[debug(skip)]
  finalize_bare_specifier: Option<Arc<FinalizeBareSpecifierCallback>>,
  #[debug(skip)]
  finalize_other_specifiers: Option<Arc<FinalizeOtherSpecifiersCallback>>,

  runtime: String,

  resolvers: Resolvers,
  external_decider: ExternalDecider,
}

impl ViteResolvePlugin {
  pub fn new(options: ViteResolveOptions) -> Self {
    let base_options = resolver::BaseOptions {
      main_fields: &options.resolve_options.main_fields,
      conditions: &options.resolve_options.conditions,
      extensions: &options.resolve_options.extensions,
      is_production: options.resolve_options.is_production,
      try_index: options.resolve_options.try_index,
      try_prefix: &options.resolve_options.try_prefix,
      as_src: options.resolve_options.as_src,
      root: &options.resolve_options.root,
      preserve_symlinks: options.resolve_options.preserve_symlinks,
    };
    let resolvers = Resolvers::new(
      &base_options,
      &options.resolve_options.external_conditions,
      options.runtime.clone(),
    );
    let no_external = Arc::new(options.no_external);
    let dedupe = Arc::new(options.dedupe.into_iter().collect());

    Self {
      external: options.external.clone(),
      no_external: Arc::clone(&no_external),
      dedupe: Arc::clone(&dedupe),
      environment_consumer: options.environment_consumer,
      environment_name: options.environment_name,
      finalize_bare_specifier: options.finalize_bare_specifier,
      finalize_other_specifiers: options.finalize_other_specifiers,
      runtime: options.runtime.clone(),
      external_decider: ExternalDecider::new(
        ExternalDeciderOptions {
          external: options.external,
          no_external: Arc::clone(&no_external),
          dedupe,
          is_build: options.resolve_options.is_build,
        },
        options.runtime,
        resolvers.get_for_external(),
      ),

      resolvers,
      resolve_options: options.resolve_options,
    }
  }

  fn name_internal(&self) -> &'static str {
    "rolldown:vite-resolve"
  }

  async fn resolve_id_internal(&self, args: &HookResolveIdArgs<'_>) -> HookResolveIdReturn {
    let scan =
      args.custom.get(&ResolveIdOptionsScan {}).is_some_and(|v| *v) || self.resolve_options.scan;

    if args.specifier.starts_with('\0')
      || args.specifier.starts_with("virtual:")
      // When injected directly in html/client code
      || args.specifier.starts_with("/virtual:")
    {
      return Ok(None);
    }

    if args.specifier.starts_with(BROWSER_EXTERNAL_ID) {
      return Ok(Some(HookResolveIdOutput { id: args.specifier.into(), ..Default::default() }));
    }

    // explicit fs paths that starts with /@fs/*
    if self.resolve_options.as_src && args.specifier.starts_with(FS_PREFIX) {
      let mut res = fs_path_from_id(args.specifier);
      // We don't need to resolve these paths since they are already resolved
      // always return here even if res doesn't exist since /@fs/ is explicit
      // if the file doesn't exist it should be a 404.
      if let Some(finalize_other_specifiers) = &self.finalize_other_specifiers {
        if let Some(finalized) = finalize_other_specifiers(&res, args.specifier).await? {
          res = finalized.into();
        }
      }
      return Ok(Some(HookResolveIdOutput { id: res.into(), ..Default::default() }));
    }

    // file url as path
    if args.specifier.starts_with("file://") {
      let path = file_url_str_to_path(args.specifier)?;
      let mut res = normalize_path(&path).into_owned();
      if let Some(finalize_other_specifiers) = &self.finalize_other_specifiers {
        if let Some(finalized) = finalize_other_specifiers(&res, args.specifier).await? {
          res = finalized;
        }
      }
      return Ok(Some(HookResolveIdOutput { id: res.into(), ..Default::default() }));
    }

    // data uri: pass through (this only happens during build and will be handled by rolldown)
    if args.specifier.trim_start().starts_with("data:") {
      return Ok(None);
    }

    if is_external_url(args.specifier) {
      return Ok(Some(HookResolveIdOutput {
        id: args.specifier.into(),
        external: Some(true.into()),
        ..Default::default()
      }));
    }

    let additional_options = AdditionalOptions::new(
      self.resolve_options.is_require.unwrap_or(args.kind == ImportKind::Require),
      self.resolve_options.prefer_relative || args.importer.is_some_and(|i| i.ends_with(".html")),
    );
    let resolver = self.resolvers.get(additional_options);

    if is_bare_import(args.specifier) {
      let external = self.resolve_options.is_build
        && self.environment_consumer == "server"
        && self.external_decider.is_external(args.specifier, args.importer);
      let result =
        resolver.resolve_bare_import(args.specifier, args.importer, external, &self.dedupe)?;
      if let Some(mut result) = result {
        if let Some(finalize_bare_specifier) = &self.finalize_bare_specifier {
          if !scan && is_in_node_modules(&result.id) {
            let finalized = finalize_bare_specifier(&result.id, args.specifier, args.importer)
              .await?
              .map(Into::into)
              .unwrap_or(result.id);
            result.id = finalized;
          }
        }

        return Ok(Some(result));
      }

      if is_builtin(args.specifier, &self.runtime) {
        if self.environment_consumer == "server" {
          if self.no_external.is_true()
              // if both noExternal and external are true, noExternal will take the higher priority and bundle it.
              // only if the id is explicitly listed in external, we will externalize it and skip this error.
              &&(matches!(self.external, ResolveOptionsExternal::True)
              || !self.external.is_external_explicitly(args.specifier))
          {
            let mut message = format!("Cannot bundle Node.js built-in \"{}\"", args.specifier);
            if let Some(importer) = args.importer {
              let current_dir =
                env::current_dir().unwrap_or(PathBuf::from(&self.resolve_options.root));
              message.push_str(&format!(
                " imported from \"{}\"",
                Path::new(importer).relative(current_dir).to_string_lossy()
              ));
            }
            message.push_str(&format!(
              ". Consider disabling environments.{}.noExternal or remove the built-in dependency.",
              self.environment_name
            ));
            return Err(anyhow!(message));
          }

          return Ok(Some(HookResolveIdOutput {
            id: args.specifier.into(),
            external: Some(true.into()),
            side_effects: Some(HookSideEffects::False),
            ..Default::default()
          }));
        } else {
          if !self.resolve_options.as_src {
            // TODO(sapphi-red): debug log
          } else if self.resolve_options.is_production {
            // TODO(sapphi-red): warn log
          }
          return Ok(Some(HookResolveIdOutput {
            id: if self.resolve_options.is_production {
              arcstr::literal!(BROWSER_EXTERNAL_ID)
            } else {
              format!("{BROWSER_EXTERNAL_ID}:{}", args.specifier).into()
            },
            ..Default::default()
          }));
        }
      }
    }

    // skip for now: https://github.com/oxc-project/oxc-resolver/pull/310
    if clean_url(args.specifier) == "/" {
      return Ok(None);
    }

    let base_dir = args
      .importer
      .map(|i| Path::new(i).parent().map(|i| i.to_str().unwrap()).unwrap_or(i))
      .unwrap_or(&self.resolve_options.root);
    let resolved = resolver.normalize_oxc_resolver_result(
      args.importer,
      &self.dedupe,
      &resolver.resolve_raw(base_dir, args.specifier),
    )?;
    if let Some(mut resolved) = resolved {
      if !scan {
        if let Some(finalize_other_specifiers) = &self.finalize_other_specifiers {
          if let Some(finalized) = finalize_other_specifiers(&resolved.id, args.specifier).await? {
            resolved.id = finalized.into();
          }
        }
      }
      return Ok(Some(resolved));
    }

    Ok(None)
  }

  async fn load_internal(&self, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    if let Some(id_without_prefix) = args.id.strip_prefix(BROWSER_EXTERNAL_ID) {
      if self.resolve_options.is_build {
        if self.resolve_options.is_production {
          // rolldown treats missing export as an error, and will break build.
          // So use cjs to avoid it.
          return Ok(Some(HookLoadOutput {
            code: "module.exports = {}".to_string(),
            ..Default::default()
          }));
        } else {
          return Ok(Some(HookLoadOutput {
            code: get_development_build_browser_external_module_code(
              // trim leading `:` if it's not empty
              if id_without_prefix.is_empty() {
                id_without_prefix
              } else {
                &id_without_prefix[1..]
              },
            ),
            ..Default::default()
          }));
        }
      } else if self.resolve_options.is_production {
        // in dev, needs to return esm
        return Ok(Some(HookLoadOutput {
          code: "export default {}".to_string(),
          ..Default::default()
        }));
      } else {
        return Ok(Some(HookLoadOutput {
          code: get_development_dev_browser_external_module_code(
            // trim leading `:` if it's not empty
            if id_without_prefix.is_empty() { id_without_prefix } else { &id_without_prefix[1..] },
          ),
          ..Default::default()
        }));
      }
    }

    if args.id.starts_with(OPTIONAL_PEER_DEP_ID) {
      if self.resolve_options.is_production {
        return Ok(Some(HookLoadOutput {
          code: "export default {}".to_string(),
          ..Default::default()
        }));
      } else {
        let [_, peer_dep, parent_dep, _] = args.id.splitn(4, ":").collect::<Vec<&str>>()[..] else {
          unreachable!()
        };
        return Ok(Some(HookLoadOutput {
          code: get_development_optional_peer_dep_module_code(peer_dep, parent_dep),
          ..Default::default()
        }));
      }
    }

    Ok(None)
  }

  fn watch_change_internal(&self, _path: &str, event: WatcherChangeKind) -> HookNoopReturn {
    // TODO(sapphi-red): we need to avoid using cache for files not watched by vite or rollup
    // https://github.com/vitejs/vite/issues/17760
    match event {
      WatcherChangeKind::Create | WatcherChangeKind::Delete => {
        self.resolvers.clear_cache();
      }
      WatcherChangeKind::Update => {}
    };
    Ok(())
  }
}

impl CallablePlugin for ViteResolvePlugin {
  fn name(&self) -> Cow<'static, str> {
    self.name_internal().into()
  }

  async fn resolve_id(&self, args: &HookResolveIdArgs<'_>) -> HookResolveIdReturn {
    self.resolve_id_internal(args).await
  }

  async fn load(&self, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    self.load_internal(args).await
  }

  async fn watch_change(&self, path: &str, event: WatcherChangeKind) -> HookNoopReturn {
    self.watch_change_internal(path, event)
  }
}

impl Plugin for ViteResolvePlugin {
  fn name(&self) -> Cow<'static, str> {
    self.name_internal().into()
  }

  async fn resolve_id(
    &self,
    _ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    self.resolve_id_internal(args).await
  }

  async fn load(&self, _ctx: &PluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    self.load_internal(args).await
  }

  async fn watch_change(
    &self,
    _ctx: &PluginContext,
    path: &str,
    event: WatcherChangeKind,
  ) -> rolldown_plugin::HookNoopReturn {
    self.watch_change_internal(path, event)
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId | HookUsage::Load | HookUsage::WatchChange
  }
}

// rolldown uses esbuild interop helper, so copy the proxy module from https://github.com/vitejs/vite/blob/main/packages/vite/src/node/optimizer/esbuildDepPlugin.ts#L259
fn get_development_build_browser_external_module_code(id_without_prefix: &str) -> String {
  format!(
    "\
module.exports = Object.create(new Proxy({{}}, {{
  get(_, key) {{
    if (
      key !== '__esModule' &&
      key !== '__proto__' &&
      key !== 'constructor' &&
      key !== 'splice'
    ) {{
      throw new Error(`Module \"{id_without_prefix}\" has been externalized for browser compatibility. Cannot access \"{id_without_prefix}.${{key}}\" in client code.  See https://vite.dev/guide/troubleshooting.html#module-externalized-for-browser-compatibility for more details.`)
    }}
  }}
}}))\
    "
  )
}
fn get_development_dev_browser_external_module_code(id_without_prefix: &str) -> String {
  format!(
    "\
export default new Proxy({{}}, {{
  get(_, key) {{
    throw new Error(`Module \"{id_without_prefix}\" has been externalized for browser compatibility. Cannot access \"{id_without_prefix}.${{key}}\" in client code.  See https://vite.dev/guide/troubleshooting.html#module-externalized-for-browser-compatibility for more details.`)
  }}
}})\
    "
  )
}
fn get_development_optional_peer_dep_module_code(peer_dep: &str, parent_dep: &str) -> String {
  format!(
    "\
throw new Error(`Could not resolve \"{peer_dep}\" imported by \"{parent_dep}\". Is it installed?`)\
    "
  )
}

fn fs_path_from_id(id: &str) -> Cow<str> {
  let fs_path = normalize_path(id.strip_prefix(FS_PREFIX).unwrap_or(id));
  if fs_path.starts_with('/') || is_windows_drive_path(&fs_path) {
    return fs_path;
  }
  format!("/{fs_path}").into()
}

fn is_external_url(id: &str) -> bool {
  if let Some(double_slash_pos) = id.find("//") {
    if double_slash_pos == 0 {
      true
    } else {
      let protocol = &id[0..double_slash_pos];
      protocol.strip_suffix(':').map(|p| p.bytes().all(|c| c.is_ascii_alphabetic())).is_some()
    }
  } else {
    false
  }
}
