use std::{borrow::Cow, path::Path};

use crate::resolver::{self, AdditionalOptions, Resolver};
use cow_utils::CowUtils;
use rolldown_common::{side_effects::HookSideEffects, ImportKind};
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, Plugin, PluginContext,
};

const BROWSER_EXTERNAL_ID: &str = "__vite-browser-external";
const OPTIONAL_PEER_DEP_ID: &str = "__vite-optional-peer-dep";
const FS_PREFIX: &str = "/@fs/";
const TS_EXTENSIONS: &[&str] = &[".ts", ".mts", ".cts", ".tsx"];

#[derive(Debug, Default)]
pub struct ViteResolveOptions {
  pub resolve_options: ViteResolveResolveOptions,
}

#[derive(Debug, Default)]
pub struct ViteResolveResolveOptions {
  pub is_production: bool,
  pub as_src: bool,
  pub prefer_relative: bool,
  pub root: String,

  pub main_fields: Vec<String>,
  pub conditions: Vec<String>,
  pub extensions: Vec<String>,
  pub try_index: bool,
  pub try_prefix: Option<String>,
  pub preserve_symlinks: bool,
}

#[derive(Debug)]
pub struct ViteResolvePlugin {
  options: ViteResolveOptions,
  resolver: Resolver,
}

impl ViteResolvePlugin {
  pub fn new(options: ViteResolveOptions) -> Self {
    Self {
      resolver: Resolver::new(&resolver::BaseOptions {
        main_fields: &options.resolve_options.main_fields,
        conditions: &options.resolve_options.conditions,
        extensions: &options.resolve_options.extensions,
        is_production: options.resolve_options.is_production,
        try_index: options.resolve_options.try_index,
        try_prefix: &options.resolve_options.try_prefix,
        as_src: options.resolve_options.as_src,
        root: &options.resolve_options.root,
        preserve_symlinks: options.resolve_options.preserve_symlinks,
      }),
      options,
    }
  }
}

impl Plugin for ViteResolvePlugin {
  fn name(&self) -> Cow<'static, str> {
    "rolldown:vite-resolve".into()
  }

  async fn resolve_id(
    &self,
    _ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    if args.specifier.starts_with('\0')
      || args.specifier.starts_with("virtual:")
      || args.specifier.starts_with("/virtual:")
    {
      return Ok(None);
    }

    if args.specifier.starts_with(BROWSER_EXTERNAL_ID) {
      // TODO: implement for dev
      return Ok(Some(HookResolveIdOutput {
        id: args.specifier.to_string(),
        ..Default::default()
      }));
    }

    if self.options.resolve_options.as_src && args.specifier.starts_with(FS_PREFIX) {
      // TODO: implement for dev
      let res = fs_path_from_id(args.specifier);
      return Ok(Some(HookResolveIdOutput { id: res.to_string(), ..Default::default() }));
    }

    if args.specifier.starts_with("file://") {
      // TODO: implement fileURLToPath properly
      let mut res = args.specifier.replace("file://", "");
      if res.starts_with('/') && is_windows_drive_path(&res[1..]) {
        res.remove(0);
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
      self.options.resolve_options.prefer_relative || args.specifier.ends_with(".html"),
      is_from_ts_importer(args.importer),
    );
    let resolver = self.resolver.get(additional_options);

    if is_bare_import(args.specifier) {
      // TODO
      return Ok(None);
    }

    let base_dir = args
      .importer
      .map(|i| Path::new(i).parent().map(|i| i.to_str().unwrap()).unwrap_or(i))
      .unwrap_or(&self.options.resolve_options.root);
    let resolved =
      normalize_oxc_resolver_result(&self.resolver, resolver.resolve(base_dir, args.specifier))?;
    if let Some(resolved) = resolved {
      // TODO: call `finalize_other_specifiers`
      return Ok(Some(resolved));
    }

    Ok(None)
  }

  async fn load(&self, _ctx: &PluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    if let Some(id_without_prefix) = args.id.strip_prefix(BROWSER_EXTERNAL_ID) {
      // TODO: implement for dev
      if self.options.resolve_options.is_production {
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
      // TODO: implement for dev
      return Ok(Some(HookLoadOutput {
        code: "export default {}".to_string(),
        ..Default::default()
      }));
    }

    Ok(None)
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

fn normalize_path(path: &str) -> Cow<str> {
  // this function does not do normalization by `path.posix.normalize`
  // but for this plugin, it is fine as we only handle paths that are absolute
  path.cow_replace('\\', "/")
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

fn is_windows_drive_path(id: &str) -> bool {
  let id_bytes = id.as_bytes();
  id_bytes.len() >= 2 && id_bytes[0].is_ascii_alphabetic() && id_bytes[1] == b':'
}

// bareImportRE.test(id)
fn is_bare_import(id: &str) -> bool {
  if is_windows_drive_path(id) {
    return false;
  }

  id.starts_with(|c| is_regex_w_character_class(c) || c == '@') && !id.contains("://")
}

// https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Regular_expressions/Character_class_escape#w
fn is_regex_w_character_class(c: char) -> bool {
  c.is_ascii_alphanumeric() || c == '_'
}

fn is_from_ts_importer(importer: Option<&str>) -> bool {
  if let Some(importer) = importer {
    // TODO: support depScan, moduleMeta
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

fn normalize_oxc_resolver_result(
  resolver: &Resolver,
  result: Result<oxc_resolver::Resolution, oxc_resolver::ResolveError>,
) -> Result<Option<HookResolveIdOutput>, oxc_resolver::ResolveError> {
  match result {
    Ok(result) => {
      let raw_path = result.full_path().to_str().unwrap().to_string();
      let path = raw_path.strip_prefix("\\\\?\\").unwrap_or(&raw_path);
      let path = normalize_path(path);

      let side_effects = result
        .package_json()
        .and_then(|pkg_json| {
          resolver.cached_package_json(pkg_json).check_side_effects_for(&raw_path)
        })
        .map(
          |side_effects| if side_effects { HookSideEffects::True } else { HookSideEffects::False },
        );
      Ok(Some(HookResolveIdOutput { id: path.into_owned(), side_effects, ..Default::default() }))
    }
    Err(oxc_resolver::ResolveError::NotFound(_id)) => {
      // TODO
      Ok(None)
    }
    Err(oxc_resolver::ResolveError::Ignored(_)) => {
      Ok(Some(HookResolveIdOutput { id: BROWSER_EXTERNAL_ID.to_string(), ..Default::default() }))
    }
    Err(err) => Err(err),
  }
}
