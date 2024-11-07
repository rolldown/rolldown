use std::borrow::Cow;

use cow_utils::CowUtils;
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, Plugin, PluginContext,
};

const BROWSER_EXTERNAL_ID: &str = "__vite-browser-external";
const OPTIONAL_PEER_DEP_ID: &str = "__vite-optional-peer-dep";
const FS_PREFIX: &str = "/@fs/";

#[derive(Debug, Default)]
pub struct ViteResolveOptions {
  pub resolve_options: ViteResolveResolveOptions,
}

#[derive(Debug, Default)]
pub struct ViteResolveResolveOptions {
  pub is_production: bool,
  pub as_src: bool,
}

#[derive(Debug, Default)]
pub struct ViteResolvePlugin {
  options: ViteResolveOptions,
}

impl ViteResolvePlugin {
  pub fn new(options: ViteResolveOptions) -> Self {
    Self { options }
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
  if fs_path.starts_with('/') {
    return fs_path;
  }
  let fs_path_bytes = fs_path.as_bytes();

  // check if fs_path matches `^[a-zA-Z]:`
  if fs_path_bytes.len() >= 2 && fs_path_bytes[0].is_ascii_alphabetic() && fs_path_bytes[1] == b':'
  {
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
