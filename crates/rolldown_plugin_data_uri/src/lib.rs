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
};

#[derive(Debug)]
pub struct ResolvedDataUri {
  pub data: ArcStr,
  pub module_type: ModuleType,
}

#[derive(Debug, Default)]
pub struct DataUriPlugin {
  resolved_data_uri: FxDashMap<String, ResolvedDataUri>,
}

impl Plugin for DataUriPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:data-uri")
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

      self
        .resolved_data_uri
        .insert(args.specifier.to_string(), ResolvedDataUri { data, module_type });

      // Return the specifier as the id to tell rolldown that this data url is handled by the plugin.
      // Don't fallback to the default resolve behavior and mark it as external.
      return Ok(Some(HookResolveIdOutput { id: args.specifier.into(), ..Default::default() }));
    }
    Ok(None)
  }

  fn load_meta(&self) -> Option<PluginHookMeta> {
    // If a `data URL` is resolved by this plugin, we want to provide the content directly without letting other plugins or rolldown to handle it.
    Some(PluginHookMeta { order: Some(PluginOrder::Pre) })
  }

  async fn load(&self, _ctx: SharedLoadPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    if is_data_url(args.id) {
      let Some(resolved) = self.resolved_data_uri.get(args.id) else {
        return Ok(None);
      };

      Ok(Some(HookLoadOutput {
        code: resolved.data.clone(),
        module_type: Some(resolved.module_type.clone()),
        ..Default::default()
      }))
    } else {
      Ok(None)
    }
  }
}
