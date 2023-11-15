use rolldown_common::ModuleId;
use rustc_hash::FxHashSet;

use crate::bundler::{
  linker::{linker::Linker, linker_info::LinkingInfoVec},
  module::ModuleVec,
  runtime::Runtime,
  utils::symbols::Symbols,
};

use super::scan_stage::ScanStageOutput;

#[derive(Debug)]
pub struct LinkStageOutput {
  pub modules: ModuleVec,
  pub entries: Vec<(Option<String>, ModuleId)>,
  pub sorted_modules: Vec<ModuleId>,
  pub linking_infos: LinkingInfoVec,
  pub symbols: Symbols,
  pub runtime: Runtime,
}

#[derive(Debug)]
pub struct LinkStage {
  pub modules: ModuleVec,
  pub entries: Vec<(Option<String>, ModuleId)>,
  pub symbols: Symbols,
  pub runtime: Runtime,
  pub sorted_modules: Vec<ModuleId>,
  pub linking_infos: LinkingInfoVec,
}

impl LinkStage {
  pub fn new(scan_stage_output: ScanStageOutput) -> Self {
    Self {
      sorted_modules: Vec::new(),
      linking_infos: LinkingInfoVec::default(),
      modules: scan_stage_output.modules,
      entries: scan_stage_output.entries,
      symbols: scan_stage_output.symbols,
      runtime: scan_stage_output.runtime,
    }
  }

  pub fn link(mut self) -> LinkStageOutput {
    self.sort_modules();
    Linker::new(&mut self).link();

    LinkStageOutput {
      modules: self.modules,
      entries: self.entries,
      sorted_modules: self.sorted_modules,
      linking_infos: self.linking_infos,
      symbols: self.symbols,
      runtime: self.runtime,
    }
  }

  pub fn sort_modules(&mut self) {
    let mut stack = self.entries.iter().map(|(_, m)| Action::Enter(*m)).rev().collect::<Vec<_>>();
    // The runtime module should always be the first module to be executed
    stack.push(Action::Enter(self.runtime.id()));
    let mut entered_ids: FxHashSet<ModuleId> = FxHashSet::default();
    entered_ids.shrink_to(self.modules.len());
    let mut sorted_modules = Vec::with_capacity(self.modules.len());
    let mut next_exec_order = 0;
    while let Some(action) = stack.pop() {
      let module = &mut self.modules[action.module_id()];
      match action {
        Action::Enter(id) => {
          if !entered_ids.contains(&id) {
            entered_ids.insert(id);
            stack.push(Action::Exit(id));
            stack.extend(
              module
                .import_records()
                .iter()
                .filter(|rec| rec.kind.is_static())
                .map(|rec| rec.resolved_module)
                .rev()
                .map(Action::Enter),
            );
          }
        }
        Action::Exit(id) => {
          *module.exec_order_mut() = next_exec_order;
          next_exec_order += 1;
          sorted_modules.push(id);
        }
      }
    }
    self.sorted_modules = sorted_modules;
    debug_assert_eq!(
      self.sorted_modules.first().copied(),
      Some(self.runtime.id()),
      "runtime module should always be the first module in the sorted modules"
    );
  }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Action {
  Enter(ModuleId),
  Exit(ModuleId),
}

impl Action {
  #[inline]
  fn module_id(&self) -> ModuleId {
    match self {
      Self::Enter(id) | Self::Exit(id) => *id,
    }
  }
}
