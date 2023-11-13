use super::{linker::Linker, linker_info::LinkingInfoVec, symbols::Symbols};
use crate::{
  bundler::{module::ModuleVec, runtime::Runtime, stages::build_stage::BuildInfo},
  error::BatchedResult,
};
use rolldown_common::ModuleId;
use rustc_hash::FxHashSet;

// TODO(hyf0): Rename this to a more meaningful name, `Graph` is too generic now.
#[derive(Default, Debug)]
pub struct Graph {
  pub modules: ModuleVec,
  pub linking_infos: LinkingInfoVec,
  pub entries: Vec<(Option<String>, ModuleId)>,
  pub sorted_modules: Vec<ModuleId>,
  pub symbols: Symbols,
  pub runtime: Runtime,
}

impl Graph {
  pub fn new(build_info: BuildInfo) -> Self {
    let BuildInfo { modules, entries, symbols, runtime } = build_info;
    Self { modules, entries, symbols, runtime, ..Default::default() }
  }

  // unnecessary_wraps(hyf0): we will add errors later in `link_inner`
  #[allow(clippy::unnecessary_wraps)]
  pub fn link(&mut self) -> BatchedResult<()> {
    self.sort_modules();

    self.link_inner();
    Ok(())
  }

  pub fn sort_modules(&mut self) {
    let mut stack = self.entries.iter().map(|(_, m)| Action::Enter(*m)).rev().collect::<Vec<_>>();
    // The runtime module should always be the first module to be executed
    stack.push(Action::Enter(self.runtime.id));
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
                .filter_map(|rec| rec.resolved_module.is_valid().then_some(rec.resolved_module))
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
      Some(self.runtime.id),
      "runtime module should always be the first module in the sorted modules"
    );
  }

  pub fn link_inner(&mut self) {
    Linker::new(self).link();
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
