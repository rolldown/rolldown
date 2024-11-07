use std::borrow::Cow;

use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdReturn, Plugin,
  PluginContext,
};

const BROWSER_EXTERNAL_ID: &str = "__vite-browser-external";
const OPTIONAL_PEER_DEP_ID: &str = "__vite-optional-peer-dep";

#[derive(Debug, Default)]
pub struct ViteResolveOptions {
  pub resolve_options: ViteResolveResolveOptions,
}

#[derive(Debug, Default)]
pub struct ViteResolveResolveOptions {
  pub is_production: bool,
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
    _args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
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
