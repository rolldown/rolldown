use std::borrow::Cow;

use rolldown_common::ModuleType;
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, Plugin, PluginContext,
};
use rolldown_utils::{
  dashmap::FxDashMap,
  dataurl::{is_data_url, parse_data_url},
};

#[derive(Debug)]
pub struct ResolvedDataUrl {
  pub module_type: ModuleType,
  pub data: String,
}

#[derive(Debug, Default)]
pub struct DataUrlPlugin {
  resolved_data_url: FxDashMap<String, ResolvedDataUrl>,
}

impl Plugin for DataUrlPlugin {
  fn name(&self) -> Cow<'static, str> {
    "rolldown:data-url".into()
  }

  fn resolve_id(
    &self,
    _ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> impl std::future::Future<Output = HookResolveIdReturn> {
    async {
      if is_data_url(args.specifier) {
        let Some(parsed) = parse_data_url(args.specifier) else {
          return Ok(None);
        };
        let decoded_data = if parsed.is_base64 {
          String::from_utf8_lossy(base64_simd::STANDARD.decode_to_vec(parsed.data)?.as_ref())
            .to_string()
        } else {
          urlencoding::decode(parsed.data)?.into_owned()
        };
        let module_type = match parsed.mime {
          "text/javascript" => ModuleType::Js,
          "application/json" => ModuleType::Json,
          "text/css" => ModuleType::Css,
          _ => {
            return Ok(None);
          }
        };

        self
          .resolved_data_url
          .insert(args.specifier.to_string(), ResolvedDataUrl { module_type, data: decoded_data });

        // Return the specifier as the id to tell rolldown that this data url is handled by the plugin. Don't fallback to
        // the default resolve behavior and mark it as external.
        return Ok(Some(HookResolveIdOutput { id: args.specifier.into(), ..Default::default() }));
      }
      Ok(None)
    }
  }

  fn load(
    &self,
    _ctx: &PluginContext,
    args: &HookLoadArgs<'_>,
  ) -> impl std::future::Future<Output = HookLoadReturn> + Send {
    async {
      if is_data_url(args.id) {
        let Some(resolved) = self.resolved_data_url.get(args.id) else {
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
}
