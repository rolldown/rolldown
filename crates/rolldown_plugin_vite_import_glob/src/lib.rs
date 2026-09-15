mod utils;

use std::{borrow::Cow, path::PathBuf};

use oxc::ast_visit::VisitJs;
use rolldown_plugin::{HookTransformOutput, HookTransformOutputMap, HookUsage, Plugin};
use rolldown_plugin_utils::parse_program;
use sugar_path::SugarPath as _;

#[derive(Debug, Default)]
pub struct ViteImportGlobPlugin {
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
    if args.code.contains("import.meta.glob") {
      let allocator = oxc::allocator::Allocator::default();
      let Some(parser_ret) = parse_program(&allocator, args.code, args.module_type, args.id)?
      else {
        return Ok(None);
      };
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
        errors: Vec::new(),
        restore_query_extension: self.restore_query_extension,
      };
      visitor.visit_program(&parser_ret.program);
      if let Some(err) = visitor.errors.into_iter().next() {
        return Err(err);
      }
      if let Some(magic_string) = visitor.magic_string {
        return Ok(Some(HookTransformOutput {
          code: Some(magic_string.to_string()),
          map: HookTransformOutputMap::from_if_enabled(self.sourcemap, || {
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
