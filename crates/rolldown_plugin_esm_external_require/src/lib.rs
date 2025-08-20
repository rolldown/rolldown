use std::borrow::Cow;

use rolldown_common::{ImportKind, IsExternal, ResolvedExternal};
use rolldown_plugin::{HookLoadOutput, HookResolveIdOutput, HookUsage, Plugin};
use rolldown_utils::concat_string;

const CJS_EXTERNAL_FACADE_PREFIX: &str = "builtin:esm-external-require-";

#[derive(Debug, Default)]
pub struct EsmExternalRequirePlugin {
  pub external: IsExternal,
}

impl Plugin for EsmExternalRequirePlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:esm-external-require")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId | HookUsage::Load
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

    if self.external.call(args.specifier, args.importer, false).await? {
      // TODO(shulaoda): Maybe we should follow
      // https://github.com/rolldown/rolldown/blob/70ab86b7/crates/rolldown_plugin/src/utils/resolve_id_check_external.rs#L31-L33
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
      let code = concat_string!("import * as m from '", module_id, "';module.exports = m;");
      HookLoadOutput { code: code.into(), ..Default::default() }
    }))
  }
}
