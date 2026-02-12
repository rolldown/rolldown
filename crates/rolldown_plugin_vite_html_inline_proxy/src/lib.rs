use std::{borrow::Cow, path::PathBuf};

use rolldown_common::side_effects::HookSideEffects;
use rolldown_plugin::{HookLoadOutput, HookResolveIdOutput, HookUsage, Plugin};
use rolldown_plugin_utils::constants::{HTMLProxyMap, HTMLProxyMapItem};
use rolldown_utils::url::clean_url;
use sugar_path::SugarPath as _;

#[derive(Debug)]
pub struct ViteHtmlInlineProxyPlugin {
  pub root: PathBuf,
}

impl Plugin for ViteHtmlInlineProxyPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:vite-html-inline-proxy")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId | HookUsage::Load
  }

  async fn resolve_id(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> rolldown_plugin::HookResolveIdReturn {
    if rolldown_plugin_utils::find_special_query(args.specifier, b"html-proxy").is_some() {
      return Ok(Some(HookResolveIdOutput::from_id(args.specifier)));
    }
    Ok(None)
  }

  async fn load(
    &self,
    ctx: rolldown_plugin::SharedLoadPluginContext,
    args: &rolldown_plugin::HookLoadArgs<'_>,
  ) -> rolldown_plugin::HookLoadReturn {
    // Pattern: /[?&]html-proxy=?(?:&inline-css)?(?:&style-attr)?&index=(\d+)\.(?:js|css)$/

    // Fast path: check if id contains html-proxy
    let Some(html_proxy_pos) = rolldown_plugin_utils::find_special_query(args.id, b"html-proxy")
    else {
      return Ok(None);
    };

    // Parse query string after html-proxy
    // Expected: html-proxy=? followed by optional &inline-css, &style-attr, then required &index=(\d+)
    let query_after_proxy = &args.id[html_proxy_pos + b"html-proxy".len()..];

    // Skip optional &inline-css and &style-attr
    let query_rest = query_after_proxy.strip_prefix("&inline-css").unwrap_or(query_after_proxy);
    let query_rest = query_rest.strip_prefix("&style-attr").unwrap_or(query_rest);

    // Now we must find &index=
    let Some(index_str_start) = query_rest.strip_prefix("&index=") else {
      return Ok(None);
    };

    // Extract digits
    let digit_end = index_str_start.as_bytes().iter().take_while(|b| b.is_ascii_digit()).count();
    if digit_end == 0 {
      return Ok(None);
    }

    let index_str = &index_str_start[..digit_end];
    let after_digits = &index_str_start[digit_end..];

    // Must be followed by .js or .css
    if after_digits != ".js" && after_digits != ".css" {
      return Ok(None);
    }

    let Ok(index) = index_str.parse::<usize>() else {
      return Err(anyhow::anyhow!("Invalid index value in HTML proxy URL: {}", args.id));
    };

    // Clean URL and normalize path
    let file = clean_url(args.id);
    let url = file.strip_prefix(self.root.to_slash_lossy().as_ref()).unwrap_or(file);

    // Get HTMLProxyMap from context metadata and find the cached result
    if let Some(html_proxy_map) =
      ctx.meta().get::<HTMLProxyMap>().expect("HTMLProxyMap is missing").inner.get(url)
    {
      if let Some(result) = html_proxy_map.get(&index) {
        let HTMLProxyMapItem { code, map } = result.value();
        return Ok(Some(HookLoadOutput {
          code: code.clone(),
          map: map.clone(),
          side_effects: Some(HookSideEffects::True),
          ..Default::default()
        }));
      }
    }

    Err(anyhow::anyhow!("No matching HTML proxy module found from {}", args.id))
  }
}
