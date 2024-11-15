use std::{borrow::Cow, future::Future, path::Path, pin::Pin, sync::Arc};

use crate::{
  external::{self, ExternalDecider, ExternalDeciderOptions},
  package_json_cache::PackageJsonCache,
  resolver::{
    self, normalize_oxc_resolver_result, resolve_bare_import, AdditionalOptions, Resolver,
  },
  utils::{is_bare_import, is_builtin, is_windows_drive_path, normalize_path, BROWSER_EXTERNAL_ID},
  CallablePlugin,
};
use derive_more::Debug;
use rolldown_common::{side_effects::HookSideEffects, ImportKind};
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, Plugin, PluginContext,
};

const OPTIONAL_PEER_DEP_ID: &str = "__vite-optional-peer-dep";
const FS_PREFIX: &str = "/@fs/";
const TS_EXTENSIONS: &[&str] = &[".ts", ".mts", ".cts", ".tsx"];

#[derive(Debug)]
pub struct ViteResolveOptions {
  pub resolve_options: ViteResolveResolveOptions,
  pub environment_consumer: String,
  pub external: external::ResolveOptionsExternal,
  pub no_external: external::ResolveOptionsNoExternal,
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
  pub is_production: bool,
  pub as_src: bool,
  pub prefer_relative: bool,
  pub root: String,

  pub main_fields: Vec<String>,
  pub conditions: Vec<String>,
  pub external_conditions: Vec<String>,
  pub extensions: Vec<String>,
  pub try_index: bool,
  pub try_prefix: Option<String>,
  pub preserve_symlinks: bool,
}

#[derive(Debug)]
pub struct ViteResolvePlugin {
  resolve_options: ViteResolveResolveOptions,
  environment_consumer: String,
  #[debug(skip)]
  pub finalize_bare_specifier: Option<Arc<FinalizeBareSpecifierCallback>>,
  #[debug(skip)]
  pub finalize_other_specifiers: Option<Arc<FinalizeOtherSpecifiersCallback>>,

  runtime: String,

  resolver: Resolver,
  package_json_cache: Arc<PackageJsonCache>,
  external_decider: ExternalDecider,
}

impl ViteResolvePlugin {
  pub fn new(options: ViteResolveOptions) -> Self {
    let package_json_cache = Arc::new(PackageJsonCache::default());
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
    let resolver = Resolver::new(&base_options);

    Self {
      environment_consumer: options.environment_consumer,
      finalize_bare_specifier: options.finalize_bare_specifier,
      finalize_other_specifiers: options.finalize_other_specifiers,
      runtime: options.runtime.clone(),
      package_json_cache: package_json_cache.clone(),
      external_decider: ExternalDecider::new(
        ExternalDeciderOptions {
          external: options.external,
          no_external: options.no_external,
          root: options.resolve_options.root.clone(),
        },
        options.runtime,
        resolver.get_for_external(&base_options, &options.resolve_options.external_conditions),
        package_json_cache.clone(),
      ),

      resolver,
      resolve_options: options.resolve_options,
    }
  }

  fn name_internal(&self) -> &'static str {
    "rolldown:vite-resolve"
  }

  async fn resolve_id_internal(&self, args: &HookResolveIdArgs<'_>) -> HookResolveIdReturn {
    if args.specifier.starts_with('\0')
      || args.specifier.starts_with("virtual:")
      || args.specifier.starts_with("/virtual:")
    {
      return Ok(None);
    }

    if args.specifier.starts_with(BROWSER_EXTERNAL_ID) {
      return Ok(Some(HookResolveIdOutput {
        id: args.specifier.to_string(),
        ..Default::default()
      }));
    }

    if self.resolve_options.as_src && args.specifier.starts_with(FS_PREFIX) {
      let mut res = fs_path_from_id(args.specifier);
      if let Some(finalize_other_specifiers) = &self.finalize_other_specifiers {
        if let Some(finalized) = finalize_other_specifiers(&res, args.specifier).await? {
          res = finalized.into();
        }
      }
      return Ok(Some(HookResolveIdOutput { id: res.to_string(), ..Default::default() }));
    }

    if args.specifier.starts_with("file://") {
      // TODO(sapphi-red): implement fileURLToPath properly
      let mut res = args.specifier.replace("file://", "");
      if res.starts_with('/') && is_windows_drive_path(&res[1..]) {
        res.remove(0);
      }
      if let Some(finalize_other_specifiers) = &self.finalize_other_specifiers {
        if let Some(finalized) = finalize_other_specifiers(&res, args.specifier).await? {
          res = finalized;
        }
      }
      return Ok(Some(HookResolveIdOutput { id: res, ..Default::default() }));
    }

    if args.specifier.trim_start().starts_with("data:") {
      return Ok(None);
    }

    if is_external_url(args.specifier) {
      return Ok(Some(HookResolveIdOutput {
        id: args.specifier.to_string(),
        external: Some(true),
        ..Default::default()
      }));
    }

    let additional_options = AdditionalOptions::new(
      args.kind == ImportKind::Require,
      self.resolve_options.prefer_relative || args.importer.map_or(false, |i| i.ends_with(".html")),
      is_from_ts_importer(args.importer),
    );
    let resolver = self.resolver.get(additional_options);

    if is_bare_import(args.specifier) {
      let external = self.environment_consumer == "server"
        && self.external_decider.is_external(args.specifier, args.importer);
      let result = resolve_bare_import(
        args.specifier,
        args.importer,
        resolver,
        &self.package_json_cache,
        &self.resolve_options.root,
        external,
      )?;
      if let Some(mut result) = result {
        if let Some(finalize_bare_specifier) = &self.finalize_bare_specifier {
          let finalized = finalize_bare_specifier(&result.id, args.specifier, args.importer)
            .await?
            .unwrap_or(result.id);
          result.id = finalized;
        }

        return Ok(Some(result));
      }

      if is_builtin(args.specifier, &self.runtime) {
        if self.environment_consumer == "server" {
          // TODO(sapphi-red): noExternal error
          return Ok(Some(HookResolveIdOutput {
            id: args.specifier.to_string(),
            external: Some(true),
            side_effects: Some(HookSideEffects::False),
          }));
        } else {
          if !self.resolve_options.as_src {
            // debug log
          } else if self.resolve_options.is_production {
            // warn log
          }
          return Ok(Some(HookResolveIdOutput {
            id: if self.resolve_options.is_production {
              BROWSER_EXTERNAL_ID.to_string()
            } else {
              format!("{BROWSER_EXTERNAL_ID}:{}", args.specifier)
            },
            ..Default::default()
          }));
        }
      }
    }

    let base_dir = args
      .importer
      .map(|i| Path::new(i).parent().map(|i| i.to_str().unwrap()).unwrap_or(i))
      .unwrap_or(&self.resolve_options.root);
    let resolved = normalize_oxc_resolver_result(
      &self.package_json_cache,
      &resolver.resolve(base_dir, args.specifier),
    )?;
    if let Some(mut resolved) = resolved {
      if let Some(finalize_other_specifiers) = &self.finalize_other_specifiers {
        if let Some(finalized) = finalize_other_specifiers(&resolved.id, args.specifier).await? {
          resolved.id = finalized;
        }
      }
      return Ok(Some(resolved));
    }

    Ok(None)
  }

  async fn load_internal(&self, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    if let Some(id_without_prefix) = args.id.strip_prefix(BROWSER_EXTERNAL_ID) {
      // TODO(sapphi-red): implement for dev
      if self.resolve_options.is_production {
        // rolldown treats missing export as an error, and will break build.
        // So use cjs to avoid it.
        return Ok(Some(HookLoadOutput {
          code: "module.exports = {}".to_string(),
          ..Default::default()
        }));
      } else {
        return Ok(Some(HookLoadOutput {
          code: get_development_browser_external_module_code(
            // trim leading `:`
            &id_without_prefix[1..],
          ),
          ..Default::default()
        }));
      }
    }

    if args.id.starts_with(OPTIONAL_PEER_DEP_ID) {
      // TODO(sapphi-red): implement for dev
      return Ok(Some(HookLoadOutput {
        code: "export default {}".to_string(),
        ..Default::default()
      }));
    }

    Ok(None)
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
}

fn get_development_browser_external_module_code(id_without_prefix: &str) -> String {
  format!(
    r#"\
module.exports = Object.create(new Proxy({{}}, {{
  get(_, key) {{
    if (
      key !== '__esModule' &&
      key !== '__proto__' &&
      key !== 'constructor' &&
      key !== 'splice'
    ) {{
      throw new Error(`Module "{id_without_prefix}" has been externalized for browser compatibility. Cannot access "{id_without_prefix}.${{key}}" in client code.  See https://vite.dev/guide/troubleshooting.html#module-externalized-for-browser-compatibility for more details.`)
    }}
  }}
}}))\
    "#
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

fn is_from_ts_importer(importer: Option<&str>) -> bool {
  if let Some(importer) = importer {
    // TODO(sapphi-red): support depScan, moduleMeta
    has_suffix(importer, TS_EXTENSIONS)
  } else {
    false
  }
}

fn has_suffix(s: &str, suffix: &[&str]) -> bool {
  if suffix.iter().any(|suffix| s.ends_with(suffix)) {
    return true;
  }

  if let Some((s, _)) = s.split_once('?') {
    suffix.iter().any(|suffix| s.ends_with(suffix))
  } else {
    false
  }
}
