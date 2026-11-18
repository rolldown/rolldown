use std::borrow::Cow;
use std::path::PathBuf;

use oxc::allocator::Allocator;
use oxc::codegen::{Codegen, CodegenOptions, CodegenReturn};
use oxc::parser::Parser;
use oxc::span::SourceType;
use oxc::transformer_plugins::{InjectGlobalVariables, InjectGlobalVariablesConfig};
use rolldown_common::{InjectImport, ModuleType};
use rolldown_ecmascript::semantic_builder_for_transform;
use rolldown_plugin::{
  HookTransformArgs, HookTransformOutput, HookTransformReturn, HookUsage, Plugin, PluginContext,
  SharedTransformPluginContext,
};
use rolldown_utils::pattern_filter::{self, StringOrRegex};
use rustc_hash::FxHashSet;

#[derive(Debug, Default)]
pub struct InjectPluginOptions {
  pub injects: Vec<InjectImport>,
  pub include: Vec<StringOrRegex>,
  pub exclude: Vec<StringOrRegex>,
}

/// Same transform as `transform.inject`, as a plugin with `include`/`exclude`,
/// like `@rollup/plugin-inject`.
#[derive(Debug)]
pub struct InjectPlugin {
  config: InjectGlobalVariablesConfig,
  /// First segment of each injected name, e.g. `Object` for `Object.assign`.
  /// A module that never spells one of them is skipped before parsing.
  names: FxHashSet<String>,
  include: Vec<StringOrRegex>,
  exclude: Vec<StringOrRegex>,
}

impl InjectPlugin {
  pub fn new(options: InjectPluginOptions) -> Self {
    let names = options
      .injects
      .iter()
      .filter_map(|inject| inject.local().split('.').next())
      .map(ToString::to_string)
      .collect();
    Self {
      config: InjectGlobalVariablesConfig::new(
        options.injects.iter().map(InjectImport::to_oxc).collect(),
      ),
      names,
      include: options.include,
      exclude: options.exclude,
    }
  }

  fn filter(&self, id: &str, ctx: &PluginContext) -> bool {
    if self.include.is_empty() && self.exclude.is_empty() {
      return true;
    }
    // Like Rollup's `createFilter`, a filter never matches an id with a null byte.
    if id.contains('\0') {
      return false;
    }
    let exclude = (!self.exclude.is_empty()).then_some(self.exclude.as_slice());
    let include = (!self.include.is_empty()).then_some(self.include.as_slice());
    let cwd = match ctx {
      // Called from JS, e.g. by Vite, there is no bundler `cwd`. Rollup's `createFilter` uses `process.cwd()` too.
      PluginContext::Napi(_) => std::env::current_dir().unwrap_or_default(),
      PluginContext::Native(_) => ctx.cwd().clone(),
    };
    pattern_filter::filter(exclude, include, id, &cwd.to_string_lossy()).inner()
  }

  fn mentions_any_name(&self, code: &str) -> bool {
    code
      .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '$' || !c.is_ascii()))
      .any(|word| self.names.contains(word))
  }
}

impl Plugin for InjectPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:inject")
  }

  async fn transform(
    &self,
    ctx: SharedTransformPluginContext,
    args: &HookTransformArgs<'_>,
  ) -> HookTransformReturn {
    let source_type = match args.module_type {
      ModuleType::Js => SourceType::mjs(),
      ModuleType::Jsx => SourceType::mjs().with_jsx(true),
      ModuleType::Ts => SourceType::mjs().with_typescript(true),
      ModuleType::Tsx => SourceType::mjs().with_typescript(true).with_jsx(true),
      _ => return Ok(None),
    };
    if !self.filter(args.id, &ctx) || !self.mentions_any_name(args.code) {
      return Ok(None);
    }

    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, args.code, source_type).parse();
    // Leave the code as it is. The error is reported when Rolldown parses the module.
    if parsed.fatal_error || !parsed.diagnostics.is_empty() {
      return Ok(None);
    }
    let mut program = parsed.program;
    let scoping = semantic_builder_for_transform().build(&program).semantic.into_scoping();
    let ret =
      InjectGlobalVariables::new(&allocator, self.config.clone()).build(scoping, &mut program);
    if !ret.changed {
      return Ok(None);
    }

    let CodegenReturn { code, map, .. } = Codegen::new()
      .with_options(CodegenOptions {
        source_map_path: Some(PathBuf::from(args.id)),
        ..CodegenOptions::default()
      })
      .build(&program);
    Ok(Some(HookTransformOutput {
      code: Some(code),
      map: map.map(|map| map.into_owned().into()).unwrap_or_default(),
      ..Default::default()
    }))
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }
}
