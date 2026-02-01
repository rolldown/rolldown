mod ast_visit;
mod ast_visit_2;
mod dynamic_import_to_glob;
mod utils;

use std::{borrow::Cow, pin::Pin, sync::Arc};

use derive_more::Debug;
use oxc::{
  ast::AstBuilder,
  ast_visit::{Visit, VisitMut},
};
use rolldown_common::ModuleType;
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, HookTransformAstArgs, HookTransformAstReturn, HookTransformOutput,
  HookUsage, Plugin, PluginContext, SharedLoadPluginContext,
};
use rolldown_utils::{
  futures::{block_on, block_on_spawn_all},
  pattern_filter::StringOrRegex,
};
use sugar_path::SugarPath as _;

pub const DYNAMIC_IMPORT_HELPER: &str = "\0rolldown_dynamic_import_helper.js";

pub type ResolverFn = dyn Fn(String, String) -> Pin<Box<dyn Future<Output = anyhow::Result<Option<String>>> + Send>>
  + Send
  + Sync;

#[derive(Debug, Default)]
pub struct ViteDynamicImportVarsPluginV2Config {
  pub sourcemap: bool,
}

#[derive(Debug, Default)]
pub struct ViteDynamicImportVarsPlugin {
  pub include: Vec<StringOrRegex>,
  pub exclude: Vec<StringOrRegex>,
  #[debug(skip)]
  pub resolver: Option<Arc<ResolverFn>>,
  pub is_v2: Option<ViteDynamicImportVarsPluginV2Config>,
}

impl Plugin for ViteDynamicImportVarsPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:vite-dynamic-import-vars")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId
      | HookUsage::Load
      | if self.is_v2.is_some() { HookUsage::Transform } else { HookUsage::TransformAst }
  }

  async fn resolve_id(
    &self,
    _ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    Ok((args.specifier == DYNAMIC_IMPORT_HELPER).then_some(HookResolveIdOutput {
      id: arcstr::literal!(DYNAMIC_IMPORT_HELPER),
      ..Default::default()
    }))
  }

  async fn load(&self, _ctx: SharedLoadPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    Ok((args.id == DYNAMIC_IMPORT_HELPER).then_some(HookLoadOutput {
      code: arcstr::literal!(include_str!("dynamic-import-helper.js")),
      ..Default::default()
    }))
  }

  async fn transform(
    &self,
    ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if !self.filter(args.id, ctx.cwd()) {
      return Ok(None);
    }
    if matches!(
      args.module_type,
      ModuleType::Js | ModuleType::Ts | ModuleType::Jsx | ModuleType::Tsx
    ) && utils::has_dynamic_import(args.code)
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
      let mut visitor = ast_visit_2::DynamicImportVarsVisit {
        ctx: &ctx,
        source_text: args.code,
        root: ctx.cwd(),
        importer: args.id.as_path(),
        need_helper: false,
        comments: &parser_ret.program.comments,
        current_comment: 0,
        async_imports: Vec::default(),
        async_imports_addrs: Vec::default(),
        magic_string: None,
      };

      visitor.visit_program(&parser_ret.program);

      if !visitor.async_imports.is_empty()
        && let Some(resolver) = &self.resolver
      {
        let async_imports = std::mem::take(&mut visitor.async_imports);
        let task = async_imports
          .into_iter()
          .map(|glob| async { resolver(glob, args.id.to_string()).await.ok()? });

        let importer = args.id.as_path().parent().unwrap();
        let result = block_on(block_on_spawn_all(task));
        for (i, item) in result.into_iter().enumerate() {
          if let Some(id) = item {
            let id = id.relative(importer);
            let id = id.to_slash_lossy();
            let id = if id.is_empty() {
              continue;
            } else if id.as_bytes()[0] == b'.' {
              id.into_owned()
            } else {
              rolldown_utils::concat_string!("./", id)
            };

            let addr = visitor.async_imports_addrs[i];
            visitor.rewrite_variable_dynamic_import(unsafe { &*addr }, Some(&id));
          }
        }
      }

      if let Some(mut magic_string) = visitor.magic_string {
        if visitor.need_helper {
          magic_string.prepend(format!(
            "import __variableDynamicImportRuntimeHelper from \"{DYNAMIC_IMPORT_HELPER}\";"
          ));
        }
        return Ok(Some(HookTransformOutput {
          code: Some(magic_string.to_string()),
          map: self.is_v2.as_ref().and_then(|config| {
            config.sourcemap.then(|| {
              magic_string.source_map(string_wizard::SourceMapOptions {
                hires: string_wizard::Hires::Boundary,
                source: args.id.into(),
                ..Default::default()
              })
            })
          }),
          ..Default::default()
        }));
      }
    }
    Ok(None)
  }

  async fn transform_ast(
    &self,
    ctx: &PluginContext,
    mut args: HookTransformAstArgs<'_>,
  ) -> HookTransformAstReturn {
    if !self.filter(args.id, args.cwd) {
      return Ok(args.ast);
    }
    args.ast.program.with_mut(|fields| {
      let source_text = fields.source.as_str();
      let ast_builder = AstBuilder::new(fields.allocator);
      let mut visitor = ast_visit::DynamicImportVarsVisit {
        ctx,
        source_text,
        ast_builder,
        root: args.cwd,
        importer: args.id.as_path(),
        need_helper: false,
        comments: &fields.program.comments,
        current_comment: 0,
        async_imports: Vec::default(),
        async_imports_addrs: Vec::default(),
      };

      visitor.visit_statements(&mut fields.program.body);

      if !visitor.async_imports.is_empty()
        && let Some(resolver) = &self.resolver
      {
        let async_imports = std::mem::take(&mut visitor.async_imports);
        let task = async_imports
          .into_iter()
          .map(|glob| async { resolver(glob, args.id.to_string()).await.ok()? });

        let importer = args.id.as_path().parent().unwrap();
        let result = block_on(block_on_spawn_all(task));
        for (i, item) in result.into_iter().enumerate() {
          if let Some(id) = item {
            let id = id.relative(importer);
            let id = id.to_slash_lossy();
            let id = if id.is_empty() {
              continue;
            } else if id.as_bytes()[0] == b'.' {
              id.into_owned()
            } else {
              rolldown_utils::concat_string!("./", id)
            };

            let addr = visitor.async_imports_addrs[i];
            visitor.rewrite_variable_dynamic_import(unsafe { &mut *addr }, Some(&id));
          }
        }
      }

      if visitor.need_helper {
        fields.program.body.push(visitor.variable_dynamic_import_runtime_helper());
      }
    });

    Ok(args.ast)
  }
}
