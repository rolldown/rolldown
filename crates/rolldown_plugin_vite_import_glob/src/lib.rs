mod utils;

use std::{borrow::Cow, path::PathBuf};

use oxc::ast_visit::Visit;
use rolldown_common::ModuleType;
use rolldown_plugin::{HookTransformOutput, HookUsage, Plugin};
use sugar_path::SugarPath as _;

#[derive(Debug, Default)]
pub struct ViteImportGlobPlugin {
  /// vite also support `source_map` config, but we can't support it now.
  /// Since the source map now follow the codegen option.
  pub root: Option<String>,
  pub sourcemap: bool,
  pub restore_query_extension: bool,
}

impl Plugin for ViteImportGlobPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:vite-import-glob")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }

  async fn transform(
    &self,
    ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if matches!(
      args.module_type,
      ModuleType::Js | ModuleType::Ts | ModuleType::Jsx | ModuleType::Tsx
    ) && args.code.contains("import.meta.glob")
    {
      let allocator = oxc::allocator::Allocator::default();
      let source_type = match args.module_type {
        ModuleType::Js => oxc::span::SourceType::mjs(),
        ModuleType::Jsx => oxc::span::SourceType::jsx(),
        ModuleType::Ts => oxc::span::SourceType::ts(),
        ModuleType::Tsx => oxc::span::SourceType::tsx(),
        _ => unreachable!(),
      };
      let parser_ret = oxc::parser::Parser::new(&allocator, args.code, source_type).parse();
      if parser_ret.panicked
        && let Some(err) =
          parser_ret.errors.iter().find(|e| e.severity == oxc::diagnostics::Severity::Error)
      {
        return Err(anyhow::anyhow!(format!(
          "Failed to parse code in '{}': {:?}",
          args.id, err.message
        )));
      }
      let id = args.id.to_slash_lossy();
      let root = self.root.as_ref().map(PathBuf::from);
      let root = root.as_ref().unwrap_or(ctx.cwd());
      let mut visitor = utils::GlobImportVisit {
        ctx: &ctx,
        root,
        id: &id,
        current: 0,
        code: args.code,
        magic_string: None,
        import_decls: Vec::new(),
        restore_query_extension: self.restore_query_extension,
      };
      visitor.visit_program(&parser_ret.program);
      if let Some(magic_string) = visitor.magic_string {
        return Ok(Some(HookTransformOutput {
          code: Some(magic_string.to_string()),
          map: self.sourcemap.then(|| {
            magic_string.source_map(string_wizard::SourceMapOptions {
              hires: string_wizard::Hires::Boundary,
              source: args.id.into(),
              ..Default::default()
            })
          }),
          ..Default::default()
        }));
      }
    }
    Ok(None)
  }
}
