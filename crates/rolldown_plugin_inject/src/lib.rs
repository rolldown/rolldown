use std::borrow::Cow;
use std::path::PathBuf;

use oxc::allocator::Allocator;
use oxc::codegen::{Codegen, CodegenOptions, CodegenReturn};
use oxc::parser::{ParseOptions, Parser};
use oxc::semantic::Scoping;
use oxc::span::SourceType;
use oxc::syntax::identifier::is_identifier_part;
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
  injects: Vec<InjectImport>,
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
      .map(|inject| inject.local().split('.').next().unwrap_or_default().to_string())
      .collect();
    Self {
      config: InjectGlobalVariablesConfig::new(
        options.injects.iter().map(InjectImport::to_oxc).collect(),
      ),
      injects: options.injects,
      names,
      include: options.include,
      exclude: options.exclude,
    }
  }

  fn filter(&self, id: &str, ctx: &PluginContext) -> bool {
    // Like Rollup's `createFilter`, skip virtual modules such as `\0rolldown/runtime.js`.
    if id.contains('\0') {
      return false;
    }
    if self.include.is_empty() && self.exclude.is_empty() {
      return true;
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
    // An escaped name like `f\u006fo` doesn't match its spelling, so parse to be sure.
    code.contains("\\u")
      || code.split(|c: char| !is_identifier_part(c)).any(|word| self.names.contains(word))
  }

  /// `transform.inject` runs after TypeScript is stripped, so a name used only as a type gets no import there.
  fn config_without_type_only_names(&self, scoping: &Scoping) -> InjectGlobalVariablesConfig {
    let unresolved = scoping.root_unresolved_references();
    let is_type_only = |inject: &&InjectImport| {
      unresolved.get(inject.local()).is_some_and(|reference_ids| {
        reference_ids.iter().all(|&id| !scoping.get_reference(id).is_value())
      })
    };
    if !self.injects.iter().any(|inject| is_type_only(&inject)) {
      return self.config.clone();
    }
    InjectGlobalVariablesConfig::new(
      self
        .injects
        .iter()
        .filter(|inject| !is_type_only(inject))
        .map(InjectImport::to_oxc)
        .collect(),
    )
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
    // Rolldown parses these as CommonJS, where an `import` statement is a syntax error.
    // Only the query is stripped, since a file name may contain `#`.
    let path = args.id.split('?').next().unwrap_or_default();
    if path.ends_with(".cjs") || path.ends_with(".cts") {
      return Ok(None);
    }
    if !self.filter(args.id, &ctx) || !self.mentions_any_name(args.code) {
      return Ok(None);
    }

    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, args.code, source_type)
      .with_options(ParseOptions { allow_return_outside_function: true, ..ParseOptions::default() })
      .parse();
    // Leave invalid code as it is, so Rolldown reports the error against the original source.
    if parsed.fatal_error || !parsed.diagnostics.is_empty() {
      return Ok(None);
    }
    let mut program = parsed.program;
    let semantic = semantic_builder_for_transform().with_check_syntax_error(true).build(&program);
    if semantic.diagnostics.has_errors() {
      return Ok(None);
    }
    let scoping = semantic.semantic.into_scoping();
    let config = if source_type.is_typescript() {
      self.config_without_type_only_names(&scoping)
    } else {
      self.config.clone()
    };
    let ret = InjectGlobalVariables::new(&allocator, config).build(scoping, &mut program);
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
