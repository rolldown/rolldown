use oxc::span::{CompactStr, Span};
use oxc_index::IndexVec;
use rolldown_common::{ModuleIdx, ResolvedExport, SymbolRef};
use rolldown_rstr::Rstr;
use rustc_hash::FxHashMap;

/// Module metadata about linking
#[derive(Debug, Default)]
pub struct DtsLinkingMetadata {
  // Store the export info for each module, including export named declaration and export star declaration.
  pub resolved_exports: FxHashMap<Rstr, ResolvedExport>,
  // pub re_export_all_names: FxHashSet<Rstr>,
  // Store the names of exclude ambiguous resolved exports.
  // It will be used to generate chunk exports and module namespace binding.
  pub sorted_and_non_ambiguous_resolved_exports: Vec<Rstr>,
  // `None` the member expression resolve to a ambiguous export.
  pub resolved_member_expr_refs: FxHashMap<Span, (Option<SymbolRef>, Vec<CompactStr>)>,
  //   pub star_exports_from_external_modules: Vec<ImportRecordIdx>,
}

pub type DtsLinkingMetadataVec = IndexVec<ModuleIdx, DtsLinkingMetadata>;
