use oxc::ast::ast::TSModuleDeclarationBody;
use oxc::codegen::{CodeGenerator, CodegenReturn, Gen};
use oxc::span::SourceType;
use oxc::transformer::{TransformOptions, Transformer};
use rolldown_common::js_regex::HybridRegex;
use rolldown_common::ModuleType;
use rolldown_ecmascript::EcmaCompiler;

use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, Plugin, SharedPluginContext,
};
use std::borrow::Cow;
use std::path::Path;

#[derive(Debug)]
enum StringOrRegex {
  String,
  Regex(HybridRegex),
}

#[derive(Debug)]
pub struct EcmaTransformPlugin {
  include: Vec<StringOrRegex>,
  exclude: Vec<StringOrRegex>,
  jsx_inject: Option<String>,
  // TODO: support transform options
}

/// only handle ecma like syntax, `jsx`,`tsx`,`ts`
impl Plugin for EcmaTransformPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("module_preload_polyfill")
  }

  async fn transform(
    &self,
    ctx: &rolldown_plugin::TransformPluginContext<'_>,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    let source_type = {
      let default_source_type = SourceType::default();
      match args.module_type {
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
        return Err(anyhow::format_err!("Error occurered when parsing {}\n: {:?}", args.id, errs));
      }
    };
    let trivias = ast.trivias.clone();
    let ret = ast.program.with_mut(move |fields| {
      let mut transformer_options = TransformOptions::default();
      match args.module_type {
        ModuleType::Jsx | ModuleType::Tsx => {
          transformer_options.react.jsx_plugin = true;
        }
        ModuleType::Ts => {}
        _ => {
          unreachable!()
        }
      }

      Transformer::new(
        fields.allocator,
        Path::new(args.id),
        source_type,
        fields.source,
        trivias,
        transformer_options,
      )
      .build(fields.program)
    });
    if !ret.errors.is_empty() {
      return Err(anyhow::anyhow!("Transform failed, got {:#?}", ret.errors));
    }
    let CodegenReturn { source_text, source_map } =
      CodeGenerator::new().enable_source_map(args.id, args.code).build(ast.program());
    let code = if let Some(ref inject) = self.jsx_inject {
      let mut ret = String::with_capacity(source_text.len() + 1 + inject.len());
      ret.push_str(inject);
      ret.push(';');
      ret.push_str(&source_text);
      ret
    } else {
      source_text
    };
    return Ok(Some(rolldown_plugin::HookTransformOutput {
      code: Some(code),
      map: source_map,
      ..Default::default()
    }));
  }
}
