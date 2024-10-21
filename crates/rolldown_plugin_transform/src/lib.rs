use oxc::{
  codegen::{CodeGenerator, CodegenOptions, CodegenReturn},
  semantic::SemanticBuilder,
  span::SourceType,
  transformer::{TransformOptions, Transformer},
};
use rolldown_common::ModuleType;
use rolldown_ecmascript::EcmaCompiler;

use oxc::transformer::{EnvOptions, Targets};
use rolldown_plugin::Plugin;
use rolldown_utils::pattern_filter::{self, StringOrRegex};
use std::borrow::Cow;
use std::path::Path;
use sugar_path::SugarPath;

#[derive(Debug, Default)]
pub struct TransformPlugin {
  pub include: Vec<StringOrRegex>,
  pub exclude: Vec<StringOrRegex>,
  pub jsx_inject: Option<String>,

  // TODO: support specific transform options. Firstly we can use `targets` but we'd better allowing user to pass more options.
  pub targets: Option<String>,
}

/// only handle ecma like syntax, `jsx`,`tsx`,`ts`
impl Plugin for TransformPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:transform")
  }

  async fn transform(
    &self,
    ctx: &rolldown_plugin::TransformPluginContext<'_>,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if !self.filter(ctx, args.id, args.module_type) {
      return Ok(None);
    }
    let source_type = {
      let mut default_source_type = SourceType::default();
      default_source_type = match args.module_type {
        ModuleType::Jsx => default_source_type.with_jsx(true),
        ModuleType::Ts => default_source_type.with_typescript(true),
        ModuleType::Tsx => default_source_type.with_typescript(true).with_jsx(true),
        _ => return Ok(None),
      };
      default_source_type
    };
    let code = args.code;
    let parse_result = EcmaCompiler::parse(args.id, code, source_type);
    let mut ast = match parse_result {
      Ok(ecma_ast) => ecma_ast,
      Err(errs) => {
        // TODO: better diagnostics handling
        return Err(anyhow::format_err!("Error occurred when parsing {}\n: {:?}", args.id, errs));
      }
    };
    let ret = ast.program.with_mut(move |fields| {
      let mut transformer_options = if let Some(targets) = &self.targets {
        TransformOptions::from_preset_env(&EnvOptions {
          targets: Targets::from_query(targets),
          ..EnvOptions::default()
        })
        .expect("Failed to create transform options")
      } else {
        TransformOptions::default()
      };
      match args.module_type {
        ModuleType::Jsx | ModuleType::Tsx => {
          transformer_options.react.jsx_plugin = true;
        }
        ModuleType::Ts => {}
        _ => {
          unreachable!()
        }
      }

      let (symbols, scopes) =
        SemanticBuilder::new().build(fields.program).semantic.into_symbol_table_and_scope_tree();
      Transformer::new(fields.allocator, Path::new(args.id), transformer_options)
        .build_with_symbols_and_scopes(symbols, scopes, fields.program)
    });
    if !ret.errors.is_empty() {
      // TODO: better error handling
      return Err(anyhow::anyhow!("Transform failed, got {:#?}", ret.errors));
    }
    let CodegenReturn { code, map } = CodeGenerator::new()
      .with_options(CodegenOptions {
        source_map_path: Some(args.id.into()),
        ..CodegenOptions::default()
      })
      .build(ast.program());
    let code = if let Some(ref inject) = self.jsx_inject {
      let mut ret = String::with_capacity(code.len() + 1 + inject.len());
      ret.push_str(inject);
      ret.push(';');
      ret.push_str(&code);
      ret
    } else {
      code
    };
    Ok(Some(rolldown_plugin::HookTransformOutput {
      code: Some(code),
      map,
      module_type: Some(ModuleType::Js),
      ..Default::default()
    }))
  }
}

impl TransformPlugin {
  fn filter(
    &self,
    ctx: &rolldown_plugin::TransformPluginContext<'_>,
    id: &str,
    module_type: &ModuleType,
  ) -> bool {
    if self.include.is_empty() && self.exclude.is_empty() {
      return matches!(module_type, ModuleType::Jsx | ModuleType::Tsx | ModuleType::Ts);
    }
    let normalized_path = Path::new(id).relative(ctx.inner.cwd());
    let normalized_id = normalized_path.to_string_lossy();
    let cleaned_id = rolldown_utils::path_ext::clean_url(&normalized_id);
    if cleaned_id == normalized_id {
      pattern_filter::filter(Some(&self.exclude), Some(&self.include), id, &normalized_id).inner()
    } else {
      pattern_filter::filter(Some(&self.exclude), Some(&self.include), id, &normalized_id).inner()
        && pattern_filter::filter(Some(&self.exclude), Some(&self.include), id, cleaned_id).inner()
    }
  }

  pub fn from_targets(targets: Option<String>) -> Self {
    Self { targets, ..Default::default() }
  }
}
