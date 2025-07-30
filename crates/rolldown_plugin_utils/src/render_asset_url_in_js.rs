use std::sync::Arc;

use rolldown_common::OutputFormat;
use rolldown_utils::url::clean_url;

use crate::{
  constants::ViteMetadata,
  to_relative_runtime_path::create_to_import_meta_url_based_relative_runtime,
};

pub struct RenderAssetUrlInJsEnv<'a> {
  format: OutputFormat,
  is_worker: bool,
  code: &'a str,
  ctx: &'a rolldown_plugin::PluginContext,
}

impl RenderAssetUrlInJsEnv<'_> {
  pub fn render_asset_url_in_js(&self) -> anyhow::Result<Option<String>> {
    // __VITE_ASSET__ -> 14 && __VITE_ASSET_PUBLIC__ -> 21
    let mut vite_asset_iter = self.code.match_indices("__VITE_ASSET_").peekable();

    if vite_asset_iter.peek().is_none() {
      return Ok(None);
    }

    let _to_relative_runtime =
      create_to_import_meta_url_based_relative_runtime(self.format, self.is_worker);

    let mut last = 0;
    let mut code = None;

    for (start, _) in vite_asset_iter {
      last = start;
      let (end, replacement) = if self.code[start + 13..].starts_with('_') {
        let start = start + 14;
        let Some((reference_id, mut end)) =
          self.code[start..].find(|c: char| !c.is_alphanumeric() && c != '&').and_then(|i| {
            self.code[start + i..]
              .starts_with("__")
              .then_some((&self.code[start..start + i], start + i + 2))
          })
        else {
          continue;
        };

        let file = self.ctx.get_file_name(reference_id)?;
        let vite_meta_data = self.ctx.meta().get::<ViteMetadata>().unwrap_or_else(|| {
          let value = Arc::new(ViteMetadata::default());
          self.ctx.meta().insert(Arc::<ViteMetadata>::clone(&value));
          value
        });

        vite_meta_data.imported_assets.insert(clean_url(&file).to_string());

        let filename = if let Some(postfix) = self.code[end..].starts_with("$_").then(|| {
          self.code[end + 2..].find("__").map_or("", |i| {
            let v = &self.code[end + 2..end + 2 + i];
            end = end + 2 + i + 2;
            v
          })
        }) {
          rolldown_utils::concat_string!(file, postfix)
        } else {
          file.to_string()
        };

        (end, filename)
      } else if self.code[start + 13..].starts_with("PUBLIC__") {
        todo!()
      } else {
        continue;
      };

      let code = code.get_or_insert_with(|| String::with_capacity(self.code.len()));
      code.push_str(&self.code[..last]);
      code.push_str(&replacement);
      last = end;
    }

    if let Some(code) = &mut code {
      if last < self.code.len() {
        code.push_str(&self.code[last..]);
      }
    }

    Ok(code)
  }
}
