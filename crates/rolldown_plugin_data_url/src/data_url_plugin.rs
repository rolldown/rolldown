use std::borrow::Cow;

use rolldown_common::ModuleType;
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, Plugin, SharedPluginContext,
};

use crate::utils::{is_data_url, parse_data_url};

#[derive(Debug)]
pub struct DataUrlPlugin;

impl DataUrlPlugin {
  async fn inner_load(
    &self,
    _ctx: &SharedPluginContext,
    args: &HookLoadArgs<'_>,
  ) -> HookLoadReturn {
    if is_data_url(args.id) {
      let Some(parsed) = parse_data_url(args.id) else {
        return Ok(None);
      };
      let decoded_data = if parsed.is_base64 {
        String::from_utf8(base64_simd::STANDARD.decode_to_vec(parsed.data)?)?
      } else {
        urlencoding::decode(parsed.data)?.into_owned()
      };
      let module_type = match parsed.mime {
        "text/javascript" => ModuleType::Js,
        "application/json" => ModuleType::Json,
        _ => {
          return Ok(None);
        }
      };

      Ok(Some(HookLoadOutput {
        code: decoded_data,
        module_type: Some(module_type),
        ..Default::default()
      }))
    } else {
      Ok(None)
    }
  }
}

#[async_trait::async_trait]
impl Plugin for DataUrlPlugin {
  fn name(&self) -> Cow<'static, str> {
    "rolldown:data-url".into()
  }

  async fn resolve_id(
    &self,
    _ctx: &SharedPluginContext,
    args: &HookResolveIdArgs,
  ) -> HookResolveIdReturn {
    if is_data_url(args.specifier) {
      return Ok(Some(HookResolveIdOutput {
        id: args.specifier.to_string(),
        ..Default::default()
      }));
    }
    Ok(None)
  }

  async fn load(&self, ctx: &SharedPluginContext, args: &HookLoadArgs) -> HookLoadReturn {
    self.inner_load(ctx, args).await
  }
}
