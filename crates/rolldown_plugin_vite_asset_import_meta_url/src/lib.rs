mod ast_utils;
mod ast_visit;
mod utils;

use std::{borrow::Cow, path::PathBuf, pin::Pin, sync::Arc};

use oxc::ast_visit::VisitMut;
use rolldown_plugin::{HookUsage, Plugin};
use rolldown_utils::dataurl::is_data_url;
use sugar_path::SugarPath as _;

pub type TryFsResolve = dyn Fn(&str) -> Pin<Box<dyn Future<Output = anyhow::Result<Option<String>>> + Send>>
  + Send
  + Sync;

#[derive(derive_more::Debug)]
pub struct ViteAssetImportMetaUrlPlugin {
  pub client_entry: String,
  #[debug(skip)]
  pub try_fs_resolve: Arc<TryFsResolve>,
}

impl Plugin for ViteAssetImportMetaUrlPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:vite-asset-import-meta-url")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }

  async fn transform(
    &self,
    ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if args.id == utils::PRELOAD_HELPER_ID
      || args.id == self.client_entry
      || !utils::contains_asset_import_meta_url(args.code)
    {
      return Ok(None);
    }

    let mut urls = Vec::new();
    let mut s: Option<string_wizard::MagicString> = None;
    let allocator = oxc::allocator::Allocator::default();

    {
      let mut parser_ret =
        oxc::parser::Parser::new(&allocator, args.code, oxc::span::SourceType::default()).parse();
      if parser_ret.panicked
        && let Some(err) =
          parser_ret.errors.iter().find(|e| e.severity == oxc::diagnostics::Severity::Error)
      {
        return Err(anyhow::anyhow!(format!(
          "Failed to parse code in '{}': {:?}",
          args.id, err.message
        )));
      }

      let mut visitor = ast_visit::NewUrlVisitor {
        urls: &mut urls,
        s: &mut s,
        code: parser_ret.program.source_text,
        ctx: &ctx,
      };
      visitor.visit_program(&mut parser_ret.program);
    }

    for (url, _span) in urls {
      if is_data_url(&url) {
        continue;
      }
      let _file = if url.starts_with('.') {
        let path = PathBuf::from(args.id).parent().unwrap().join(&url).normalize();
        let file = path.to_slash_lossy().into_owned();
        (self.try_fs_resolve)(&file).await?.unwrap_or(file)
      } else {
        todo!();
      };
      todo!()
    }

    // TODO: generate source map
    Ok(s.map(|s| rolldown_plugin::HookTransformOutput {
      code: Some(s.to_string()),
      ..Default::default()
    }))
  }
}
