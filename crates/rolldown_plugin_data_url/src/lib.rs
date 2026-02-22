use std::borrow::Cow;

use arcstr::ArcStr;
use rolldown_common::ModuleType;
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, HookUsage, Plugin, PluginContext, PluginHookMeta, PluginOrder,
  SharedLoadPluginContext,
};
use rolldown_utils::{
  dashmap::FxDashMap,
  dataurl::{is_data_url, parse_data_url},
  xxhash::xxhash_base64_url,
};

#[derive(Debug)]
pub struct ResolvedDataUrl {
  pub data: ArcStr,
  pub module_type: ModuleType,
}

#[derive(Debug, Default)]
pub struct DataUrlPlugin {
  resolved_data_url: FxDashMap<ArcStr, ResolvedDataUrl>,
}

impl Plugin for DataUrlPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:data-url")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId | HookUsage::Load
  }

  fn resolve_id_meta(&self) -> Option<PluginHookMeta> {
    // Users might have other plugins to handle data URLs, we should give them a chance to do so by resolving data URLs as late as possible.
    Some(PluginHookMeta { order: Some(PluginOrder::PinPost) })
  }

  async fn resolve_id(
    &self,
    _ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    if is_data_url(args.specifier) {
      let Some(parsed) = parse_data_url(args.specifier) else {
        return Ok(None);
      };

      let module_type = match parsed.mime {
        "text/css" => ModuleType::Css,
        "text/javascript" => ModuleType::Js,
        "application/json" => ModuleType::Json,
        _ => {
          return Ok(None);
        }
      };

      let data = if parsed.is_base64 {
        let data = base64_simd::STANDARD.decode_to_vec(parsed.data)?;
        simdutf8::basic::from_utf8(&data)?;
        // SAFETY: `data` is valid utf8
        unsafe { String::from_utf8_unchecked(data) }.into()
      } else {
        urlencoding::decode(parsed.data)?.as_ref().into()
      };

      let id: ArcStr =
        format!("\0rolldown/data-url:{}", xxhash_base64_url(args.specifier.as_bytes())).into();

      self.resolved_data_url.insert(id.clone(), ResolvedDataUrl { data, module_type });

      return Ok(Some(HookResolveIdOutput { id, ..Default::default() }));
    }
    Ok(None)
  }

  fn load_meta(&self) -> Option<PluginHookMeta> {
    // If a `data URL` is resolved by this plugin, we want to provide the content directly without letting other plugins or rolldown to handle it.
    Some(PluginHookMeta { order: Some(PluginOrder::Pre) })
  }

  async fn load(&self, _ctx: SharedLoadPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    let Some(resolved) = self.resolved_data_url.get(args.id) else {
      return Ok(None);
    };

    Ok(Some(HookLoadOutput {
      code: resolved.data.clone(),
      module_type: Some(resolved.module_type.clone()),
      ..Default::default()
    }))
  }
}
