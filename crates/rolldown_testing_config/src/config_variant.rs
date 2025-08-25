use rolldown_common::{
  BundlerOptions, ExperimentalOptions, OptimizationOption, OutputExports, OutputFormat,
  PreserveEntrySignatures, TreeshakeOptions,
};
use schemars::JsonSchema;
use serde::Deserialize;
use std::fmt::Write;

#[derive(Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigVariant {
  pub format: Option<OutputFormat>,
  pub extend: Option<bool>,
  /// If specified, it will be set to `BundlerOptions.name` for the variant.
  pub name: Option<String>,
  pub exports: Option<OutputExports>,
  pub strict_execution_order: Option<bool>,
  pub entry_filenames: Option<String>,
  pub inline_dynamic_imports: Option<bool>,
  pub preserve_entry_signatures: Option<PreserveEntrySignatures>,
  pub treeshake: Option<TreeshakeOptions>,
  pub minify_internal_exports: Option<bool>,
  pub on_demand_wrapping: Option<bool>,
  pub profiler_names: Option<bool>,
  pub pife_for_module_wrappers: Option<bool>,
  pub top_level_var: Option<bool>,
  // --- non-bundler options are start with `_`
  /// Whether to include the output in the snapshot for this config variant.
  #[serde(rename = "_snapshot")]
  pub snapshot: Option<bool>,
  #[serde(rename = "_configName")]
  pub config_name: Option<String>,
}

impl ConfigVariant {
  pub fn apply(&self, config: &rolldown_common::BundlerOptions) -> BundlerOptions {
    let mut config = config.clone();
    if let Some(format) = &self.format {
      config.format = Some(*format);
    }
    if let Some(exports) = &self.exports {
      config.exports = Some(*exports);
    }
    if let Some(extend) = &self.extend {
      config.extend = Some(*extend);
    }
    if let Some(name) = &self.name {
      config.name = Some(name.to_string());
    }
    if let Some(strict_execution_order) = &self.strict_execution_order {
      config.experimental.get_or_insert_default().strict_execution_order =
        Some(*strict_execution_order);
    }
    if let Some(entry_filenames) = &self.entry_filenames {
      config.entry_filenames = Some(entry_filenames.to_string().into());
    }
    if let Some(inline_dynamic_imports) = &self.inline_dynamic_imports {
      config.inline_dynamic_imports = Some(*inline_dynamic_imports);
    }
    if let Some(preserve_entry_signatures) = &self.preserve_entry_signatures {
      config.preserve_entry_signatures = Some(*preserve_entry_signatures);
    }
    if let Some(treeshake) = &self.treeshake {
      config.treeshake = treeshake.clone();
    }
    if let Some(minify_internal_exports) = &self.minify_internal_exports {
      config.minify_internal_exports = Some(*minify_internal_exports);
    }
    if let Some(on_demand_wrapping) = &self.on_demand_wrapping {
      config.experimental = Some(ExperimentalOptions {
        on_demand_wrapping: Some(*on_demand_wrapping),
        ..config.experimental.unwrap_or_default()
      });
    }
    if let Some(profiler_names) = &self.profiler_names {
      config.profiler_names = Some(*profiler_names);
    }
    if let Some(top_level_var) = &self.top_level_var {
      config.top_level_var = Some(*top_level_var);
    }
    if let Some(pife_for_module_wrappers) = &self.pife_for_module_wrappers {
      config.optimization = Some(OptimizationOption {
        pife_for_module_wrappers: Some(*pife_for_module_wrappers),
        ..config.optimization.unwrap_or_default()
      });
    }
    config
  }

  pub fn description(&self) -> String {
    let mut fields = vec![];
    if let Some(format) = &self.format {
      fields.push(format!("format: {format:?}"));
    }
    if let Some(extend) = &self.extend {
      fields.push(format!("extend: {extend:?}"));
    }
    if let Some(name) = &self.name {
      fields.push(format!("name: {name:?}"));
    }
    if let Some(exports) = &self.exports {
      fields.push(format!("exports: {exports:?}"));
    }
    if let Some(strict_execution_order) = &self.strict_execution_order {
      fields.push(format!("strict_execution_order: {strict_execution_order:?}"));
    }
    if let Some(inline_dynamic_imports) = &self.inline_dynamic_imports {
      fields.push(format!("inline_dynamic_imports: {inline_dynamic_imports:?}"));
    }
    if let Some(preserve_entry_signatures) = &self.preserve_entry_signatures {
      fields.push(format!("preserve_entry_signatures: {preserve_entry_signatures:?}"));
    }
    if let Some(treeshake) = &self.treeshake {
      fields.push(format!("treeshake: {treeshake:?}"));
    }
    if let Some(on_demand_wrapping) = &self.on_demand_wrapping {
      fields.push(format!("on_demand_wrapping: {on_demand_wrapping:?}"));
    }
    if let Some(pife_for_module_wrappers) = &self.pife_for_module_wrappers {
      fields.push(format!("pife_for_module_wrappers: {pife_for_module_wrappers:?}"));
    }
    if let Some(profiler_names) = &self.profiler_names {
      fields.push(format!("profiler_names: {profiler_names:?}"));
    }
    if let Some(entry_filenames) = &self.entry_filenames {
      fields.push(format!("entry_filenames: {entry_filenames:?}"));
    }
    if let Some(top_level_var) = &self.top_level_var {
      fields.push(format!("top_level_var: {top_level_var:?}"));
    }
    let mut result = String::new();
    self.config_name.as_ref().inspect(|config_name| {
      write!(result, "{config_name}: ").unwrap();
    });
    fields.sort();
    result.push_str(&fields.iter().map(|field| format!("[{field}]")).collect::<Vec<_>>().join(" "));
    result
  }
}
