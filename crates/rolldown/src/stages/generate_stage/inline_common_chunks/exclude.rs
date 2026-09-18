use itertools::Itertools;
use rolldown_common::{MatchGroupTest, Module};
use rolldown_error::BuildResult;
use rustc_hash::FxHashSet;

use super::InlineCommonChunksState;
use crate::stages::generate_stage::GenerateStage;

const DECLARATION_FILE_SUFFIXES: &[&str] = &[".d.ts", ".d.mts", ".d.cts"];

impl GenerateStage<'_> {
  /// Evaluates `exclude` once for every included module. The `exclude` function is an async JS
  /// callback while the selection point is synchronous, so the answers are computed here, at the
  /// start of the generate stage, and read later.
  pub(in crate::stages::generate_stage) async fn prepare_inline_common_chunks(
    &mut self,
  ) -> BuildResult<()> {
    let Some(options) = self.options.inline_common_chunks.as_ref().filter(|o| o.is_enabled())
    else {
      return Ok(());
    };
    let runtime_idx = self.link_output.runtime.id();
    let candidates = self
      .link_output
      .module_table
      .modules
      .iter()
      .filter_map(Module::as_normal)
      .filter(|module| module.idx != runtime_idx && self.link_output.metas[module.idx].is_included)
      .sorted_unstable_by(|a, b| a.stable_id.cmp(&b.stable_id))
      .collect_vec();

    let mut excluded_modules = FxHashSet::default();
    for module in &candidates {
      if DECLARATION_FILE_SUFFIXES.iter().any(|suffix| module.id.as_str().ends_with(suffix)) {
        excluded_modules.insert(module.idx);
      }
    }
    for test in &options.exclude {
      match test {
        MatchGroupTest::Regex(regex) => {
          for module in &candidates {
            if regex.matches(&module.id) {
              excluded_modules.insert(module.idx);
            }
          }
        }
        MatchGroupTest::Function(func) => {
          let module_ids = candidates.iter().map(|module| module.id.to_string()).collect_vec();
          let results = func(module_ids).await?;
          if results.len() != candidates.len() {
            return Err(
              anyhow::anyhow!(
                "`output.codeSplitting.experimentalInlineCommonChunks.exclude` returned {} results for {} modules",
                results.len(),
                candidates.len()
              )
              .into(),
            );
          }
          for (module, matched) in candidates.iter().zip(results) {
            if matched {
              excluded_modules.insert(module.idx);
            }
          }
        }
      }
    }
    self.inline_state = InlineCommonChunksState::with_excluded_modules(excluded_modules);
    Ok(())
  }
}
