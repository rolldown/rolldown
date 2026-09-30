use itertools::Itertools;
use rolldown_common::Module;
use rolldown_error::BuildResult;
use rustc_hash::FxHashSet;

use super::InlineCommonChunksState;
use crate::{stages::generate_stage::GenerateStage, utils::module_id_matcher::match_module_ids};

/// `rolldown-plugin-dts` builds declaration output from fake JavaScript modules with these ids;
/// its `renderChunk` deletes the statements it does not recognize, so a factory printed into one
/// would vanish and leave its readers without a registration.
const DECLARATION_FILE_SUFFIXES: &[&str] = &[".d.ts", ".d.mts", ".d.cts"];

impl GenerateStage<'_> {
  /// Evaluates `exclude` once for every included module. The `exclude` function is an async JS
  /// callback while the selection point is synchronous, so the answers are computed here, at the
  /// start of the generate stage, and read later.
  pub(in crate::stages::generate_stage) async fn prepare_inline_common_chunks(
    &mut self,
  ) -> BuildResult<()> {
    let Some(options) = &self.options.inline_common_chunks else {
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
    let ids = candidates.iter().map(|module| module.id.as_str()).collect_vec();
    for test in &options.exclude {
      let matched = match_module_ids(
        test,
        &ids,
        "`output.codeSplitting.experimentalInlineCommonChunks.exclude`",
      )
      .await?;
      for (module, matched) in candidates.iter().zip(matched) {
        if matched {
          excluded_modules.insert(module.idx);
        }
      }
    }
    self.inline_state = InlineCommonChunksState::with_excluded_modules(excluded_modules);
    Ok(())
  }
}
