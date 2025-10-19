use std::path::Path;

use crate::inner_bundler_options::types::output_option::PathsOutputOption;
use crate::side_effects::DeterminedSideEffects;
use crate::{Chunk, ImportRecordIdx, ModuleIdx, ResolvedImportRecord, SymbolRef};
use arcstr::ArcStr;
use oxc_index::IndexVec;
use rolldown_utils::concat_string;
use sugar_path::SugarPath;

#[derive(Debug, Clone)]
pub struct ExternalModule {
  pub idx: ModuleIdx,
  pub exec_order: u32,
  /// Usages:
  /// - Used for iife format to inject symbol and deconflict.
  /// - Used for for rewrite `import { foo } from 'external';console.log(foo)` to `var external = require('external'); console.log(external.foo)` in cjs format.
  pub namespace_ref: SymbolRef,
  // The resolved id of the external module. It could be an absolute path or a relative path.
  // If resolved id `external` is `true`, the absolute ids will be converted to relative ids based on the `makeAbsoluteExternalsRelative` option
  pub id: ArcStr,
  // Similar to the rollup `ExternalChunk#get_file_name`, It could be an absolute path or a normalized relative path.
  pub name: ArcStr,
  pub identifier_name: ArcStr,
  pub import_records: IndexVec<ImportRecordIdx, ResolvedImportRecord>,
  pub side_effects: DeterminedSideEffects,
  pub need_renormalize_render_path: bool,
}

impl ExternalModule {
  pub fn new(
    idx: ModuleIdx,
    id: ArcStr,
    name: ArcStr,
    identifier_name: ArcStr,
    side_effects: DeterminedSideEffects,
    namespace_ref: SymbolRef,
    need_renormalize_render_path: bool,
  ) -> Self {
    Self {
      idx,
      id,
      exec_order: u32::MAX,
      namespace_ref,
      name,
      identifier_name,
      import_records: IndexVec::default(),
      side_effects,
      need_renormalize_render_path,
    }
  }

  pub fn get_file_name(&self, paths: Option<&PathsOutputOption>) -> ArcStr {
    // Try to apply paths mapping first
    if let Some(paths_option) = paths {
      if let Some(mapped_path) = paths_option.call(&self.id) {
        return mapped_path.into();
      }
    }
    self.name.clone()
  }

  pub fn get_import_path(&self, chunk: &Chunk, paths: Option<&PathsOutputOption>) -> ArcStr {
    if !self.need_renormalize_render_path {
      return self.get_file_name(paths);
    }
    let file_name = self.get_file_name(paths);
    let mut target = file_name.as_str();
    let mut importer = chunk
      .preliminary_filename
      .as_deref()
      .expect("importer chunk should have preliminary_filename")
      .to_string();
    while target.starts_with("../") {
      target = &target[3..];
      importer = concat_string!("_/", importer);
    }
    let relative_path = Path::new(target).relative(
      importer
        .as_path()
        .parent()
        .expect("the importer chunk preliminary filename should have a parent directory"),
    );
    if relative_path.starts_with("..") {
      relative_path.to_slash_lossy().into()
    } else if relative_path.to_string_lossy().is_empty() {
      ".".into()
    } else {
      concat_string!("./", relative_path.to_slash_lossy()).into()
    }
  }
}
