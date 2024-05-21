use oxc::span::CompactStr;
use oxc_index::IndexVec;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{ImportRecordId, SymbolRef};

#[derive(Debug, Default)]
pub struct StmtInfos {
  infos: IndexVec<StmtInfoId, StmtInfo>,
  // only for top level symbols
  symbol_ref_to_declared_stmt_idx: FxHashMap<SymbolRef, Vec<StmtInfoId>>,
}

impl StmtInfos {
  pub fn get(&self, id: StmtInfoId) -> &StmtInfo {
    &self.infos[id]
  }

  pub fn get_mut(&mut self, id: StmtInfoId) -> &mut StmtInfo {
    &mut self.infos[id]
  }

  pub fn add_stmt_info(&mut self, info: StmtInfo) -> StmtInfoId {
    let id = self.infos.push(info);
    for symbol_ref in &self.infos[id].declared_symbols {
      self.symbol_ref_to_declared_stmt_idx.entry(*symbol_ref).or_default().push(id);
    }
    id
  }

  pub fn replace_namespace_stmt_info(&mut self, info: StmtInfo) -> StmtInfoId {
    self.infos[0] = info;
    for symbol_ref in &self.infos[0].declared_symbols {
      self
        .symbol_ref_to_declared_stmt_idx
        .entry(*symbol_ref)
        .or_default()
        .push(StmtInfoId::from_raw(0));
    }
    StmtInfoId::from_raw(0)
  }

  pub fn declared_stmts_by_symbol(&self, symbol_ref: &SymbolRef) -> &[StmtInfoId] {
    self.symbol_ref_to_declared_stmt_idx.get(symbol_ref).map_or(&[], Vec::as_slice)
  }
}

impl std::ops::Deref for StmtInfos {
  type Target = IndexVec<StmtInfoId, StmtInfo>;

  fn deref(&self) -> &Self::Target {
    &self.infos
  }
}

impl std::ops::DerefMut for StmtInfos {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.infos
  }
}

oxc_index::define_index_type! {
  pub struct StmtInfoId = u32;
}

#[derive(Debug, Clone, Default)]
pub enum IncludedInfo {
  #[default]
  False,
  True,
  Declarator(FxHashSet<CompactStr>),
}

#[derive(Default, Debug)]
pub struct StmtInfo {
  /// The index of this statement in the module body.
  ///
  /// We will create some facade statements while bundling, and the facade statements
  /// don't have a corresponding statement in the original module body, which means
  /// `stmt_idx` will be `None`.
  pub stmt_idx: Option<usize>,
  // currently, we only store top level symbols
  pub declared_symbols: Vec<SymbolRef>,
  // We will add symbols of other modules to `referenced_symbols`, so we need `SymbolRef`
  // here instead of `SymbolId`.
  /// Top level symbols referenced by this statement.
  pub referenced_symbols: Vec<SymbolRef>,
  pub side_effect: bool,
  pub included_info: IncludedInfo,
  pub import_records: Vec<ImportRecordId>,
  pub debug_label: Option<String>,
}

impl StmtInfo {
  pub fn to_debug_stmt_info_for_tree_shaking(&self) -> DebugStmtInfoForTreeShaking {
    DebugStmtInfoForTreeShaking {
      is_included: self.partial_included(),
      side_effect: self.side_effect,
      source: self.debug_label.clone().unwrap_or_else(|| "<Noop>".into()),
    }
  }

  pub fn partial_included(&self) -> bool {
    match self.included_info {
      IncludedInfo::False => false,
      IncludedInfo::True => true,
      IncludedInfo::Declarator(ref set) => !set.is_empty(),
    }
  }

  pub fn fully_included(&self) -> bool {
    match self.included_info {
      IncludedInfo::False => false,
      IncludedInfo::True => true,
      IncludedInfo::Declarator(ref set) => {
        set.len() == self.declared_symbols.len()
        // self.declared_symbols.iter().all(|symbol_ref| set.contains(symbol_ref))
      }
    }
  }

  // pub fn (&self) -> bool {
  //   match self.included_info {
  //     IncludedInfo::False => false,
  //     IncludedInfo::True => true,
  //     IncludedInfo::Declarator(ref set) => {
  //       self.declared_symbols.iter().all(|symbol_ref| set.contains(symbol_ref))
  //     }
  //   }
  // }
}

#[derive(Debug)]
pub struct DebugStmtInfoForTreeShaking {
  pub is_included: bool,
  pub side_effect: bool,
  pub source: String,
}
