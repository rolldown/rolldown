use oxc::span::Atom;
use rolldown_common::{ModuleId, SymbolRef};

use rustc_hash::FxHashMap;

use crate::bundler::{
  linker::linker_info::{LinkingInfo, LinkingInfoVec},
  module::{ModuleVec, NormalModule},
  runtime::RuntimeModuleBrief,
  utils::symbols::Symbols,
};

pub struct FinalizerContext<'me> {
  pub id: ModuleId,
  pub module: &'me NormalModule,
  pub modules: &'me ModuleVec,
  pub linking_info: &'me LinkingInfo,
  pub linking_infos: &'me LinkingInfoVec,
  pub symbols: &'me Symbols,
  pub canonical_names: &'me FxHashMap<SymbolRef, Atom>,
  pub runtime: &'me RuntimeModuleBrief,
}
