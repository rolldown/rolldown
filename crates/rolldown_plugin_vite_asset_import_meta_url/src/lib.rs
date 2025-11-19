mod ast_utils;
mod ast_visit;
mod utils;

use std::borrow::Cow;

use oxc::ast_visit::VisitMut;
use rolldown_plugin::{HookUsage, Plugin};

#[derive(Debug)]
pub struct ViteAssetImportMetaUrlPlugin {
  pub client_entry: String,
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

    let allocator = oxc::allocator::Allocator::default();
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

    let mut s: Option<string_wizard::MagicString> = None;

    let mut visitor = ast_visit::NewUrlVisitor {
      urls: Vec::new(),
      s: &mut s,
      code: parser_ret.program.source_text,
      ctx: &ctx,
    };
    visitor.visit_program(&mut parser_ret.program);

    // TODO: generate source map
    Ok(s.map(|s| rolldown_plugin::HookTransformOutput {
      code: Some(s.to_string()),
      ..Default::default()
    }))
  }
}
