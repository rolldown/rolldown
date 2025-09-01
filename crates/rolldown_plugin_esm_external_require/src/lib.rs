use std::borrow::Cow;

use rolldown_common::{ImportKind, ResolvedExternal, is_existing_node_builtin_modules};
use rolldown_plugin::{HookLoadOutput, HookResolveIdOutput, HookUsage, Plugin};
use rolldown_utils::{concat_string, pattern_filter::StringOrRegex};

const CJS_EXTERNAL_FACADE_PREFIX: &str = "builtin:esm-external-require-";

#[derive(Debug, Default)]
pub struct EsmExternalRequirePlugin {
  pub external: Vec<StringOrRegex>,
}

impl Plugin for EsmExternalRequirePlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:esm-external-require")
  }

  fn register_hook_usage(&self) -> HookUsage {
    if self.external.is_empty() {
      HookUsage::empty()
    } else {
      HookUsage::ResolveId | HookUsage::Load
    }
  }

  async fn resolve_id(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> rolldown_plugin::HookResolveIdReturn {
    if args.importer.is_some_and(|importer| importer.starts_with(CJS_EXTERNAL_FACADE_PREFIX)) {
      return Ok(Some(HookResolveIdOutput {
        id: args.specifier.into(),
        external: Some(ResolvedExternal::Bool(true)),
        ..Default::default()
      }));
    }

    let is_external = self.external.iter().any(|v| match v {
      StringOrRegex::String(string) => string == args.specifier,
      StringOrRegex::Regex(regex) => regex.matches(args.specifier),
    });

    if is_external {
      if !ctx.options().format.is_esm() || args.kind != ImportKind::Require {
        return Ok(Some(HookResolveIdOutput {
          id: args.specifier.into(),
          external: Some(ResolvedExternal::Bool(true)),
          ..Default::default()
        }));
      }

      return Ok(Some(HookResolveIdOutput {
        id: concat_string!(CJS_EXTERNAL_FACADE_PREFIX, args.specifier).into(),
        ..Default::default()
      }));
    }

    Ok(None)
  }

  async fn load(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookLoadArgs<'_>,
  ) -> rolldown_plugin::HookLoadReturn {
    Ok(args.id.strip_prefix(CJS_EXTERNAL_FACADE_PREFIX).map(|module_id| {
      let code = concat_string!(
        "import * as m from '",
        module_id,
        "';module.exports = ",
        if is_existing_node_builtin_modules(module_id) { "m.default" } else { "{ ...m }" },
        ";"
      );
      HookLoadOutput { code: code.into(), ..Default::default() }
    }))
  }
}
