use std::path::Path;

use crate::side_effects::DeterminedSideEffects;
use crate::{Chunk, ImportRecordIdx, ModuleIdx, ResolvedImportRecord, SymbolRef};
use arcstr::ArcStr;
use oxc_index::IndexVec;
use rolldown_utils::concat_string;
use sugar_path::SugarPath;

#[derive(Debug)]
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

  pub fn get_import_path(&self, chunk: &Chunk) -> ArcStr {
    if !self.need_renormalize_render_path {
      return self.name.clone();
    }
    let mut target = self.name.as_str();
    println!("target: {target:?}");
    let mut importer = chunk
      .preliminary_filename
      .as_deref()
      .expect("importer chunk should have preliminary_filename")
      .to_string();
    while target.starts_with("../") {
      target = &target[3..];
      importer = concat_string!("_/", importer);
    }
    let dirname = Path::new(&importer)
      .parent()
      .expect("the importer chunk preliminary filename should have a parent directory");
    println!("dirname: {dirname:?}");
    let cwd = std::env::current_dir().unwrap();
    println!("cwd: {cwd:?}");
    let relative_path = target
      .as_path()
      .relative(
        importer
          .as_path()
          .parent()
          .expect("the importer chunk preliminary filename should have a parent directory"),
      )
      .normalize();
    let relative_path = relative_path.to_slash_lossy();
    println!("importer : {importer:?}");
    println!("relative_path: {relative_path:?}");
    if relative_path.is_empty() {
      let target_basename = Path::new(target).file_name().and_then(|s| s.to_str()).unwrap_or("");
      return concat_string!("../", target_basename).into();
    }

    if relative_path.starts_with("..") {
      relative_path.to_slash_lossy().into()
    } else {
      concat_string!("./", relative_path.to_slash_lossy()).into()
    }
  }
}
