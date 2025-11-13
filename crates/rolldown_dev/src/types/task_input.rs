use std::path::PathBuf;

use rolldown_utils::indexmap::FxIndexSet;

#[derive(Debug)]
pub enum TaskInput {
  /// A full build
  FullBuild,
  /// Incremental rebuild only (no HMR updates)
  Rebuild { changed_files: FxIndexSet<PathBuf> },
  /// Generate HMR updates only (no rebuild)
  Hmr { changed_files: FxIndexSet<PathBuf> },
  /// Generate HMR updates AND rebuild
  HmrRebuild { changed_files: FxIndexSet<PathBuf> },
}

impl TaskInput {
  #[expect(dead_code)]
  pub fn new_initial_build_task() -> Self {
    Self::FullBuild
  }

  pub fn changed_files(&self) -> &FxIndexSet<PathBuf> {
    match self {
      Self::FullBuild => {
        use std::sync::OnceLock;
        static EMPTY: OnceLock<FxIndexSet<PathBuf>> = OnceLock::new();
        EMPTY.get_or_init(FxIndexSet::default)
      }
      Self::Rebuild { changed_files }
      | Self::Hmr { changed_files }
      | Self::HmrRebuild { changed_files } => changed_files,
    }
  }

  pub fn changed_files_mut(&mut self) -> Option<&mut FxIndexSet<PathBuf>> {
    match self {
      Self::FullBuild => None,
      Self::Rebuild { changed_files }
      | Self::Hmr { changed_files }
      | Self::HmrRebuild { changed_files } => Some(changed_files),
    }
  }

  pub fn requires_full_rebuild(&self) -> bool {
    matches!(self, Self::FullBuild)
  }

  pub fn require_generate_hmr_update(&self) -> bool {
    matches!(self, Self::Hmr { .. } | Self::HmrRebuild { .. })
  }

  pub fn requires_rebuild(&self) -> bool {
    matches!(self, Self::FullBuild | Self::Rebuild { .. } | Self::HmrRebuild { .. })
  }

  pub fn is_mergeable_with(&self, other: &Self) -> bool {
    match self {
      // FullRebuild absorbs everything
      // - Incoming hmr update task would be meaningless, because full rebuild will bundle with latest disk files' contents.
      // - The build output will contains latest contents, it's no need to and we can't generate hmr updates for such situation.
      // - The incoming incremental rebuild task would be meaningless, because the build output will contains latest contents.
      Self::FullBuild => true,
      // Rebuild only task can only merge with other rebuild only task.
      // If we merge a hmr update task, we'll involve files that're not intend to be involved in the hmr generation.
      Self::Rebuild { .. } => matches!(other, Self::Rebuild { .. }),
      // Hmr update task can only merge with other Hmr update task (include hmr with incremental rebuild).
      Self::Hmr { .. } | Self::HmrRebuild { .. } => {
        matches!(other, Self::Hmr { .. } | Self::HmrRebuild { .. })
      }
    }
  }

  // You should call `is_mergeable_with` first to check if the two tasks are mergeable in business logic.
  pub fn merge_with(&mut self, other: Self) {
    match (self, other) {
      // FullRebuild absorbs everything and stays the same
      (Self::FullBuild, _) => {}
      // Rebuild + Rebuild = Rebuild with merged files
      // Hmr + Hmr = Hmr with merged files
      // HmrRebuild + Hmr = HmrRebuild with merged files
      // HmrRebuild + HmrRebuild = HmrRebuild with merged files
      (Self::Rebuild { changed_files }, Self::Rebuild { changed_files: other_files })
      | (
        Self::Hmr { changed_files } | Self::HmrRebuild { changed_files },
        Self::Hmr { changed_files: other_files },
      )
      | (Self::HmrRebuild { changed_files }, Self::HmrRebuild { changed_files: other_files }) => {
        changed_files.extend(other_files);
      }
      // Hmr + HmrRebuild = HmrRebuild with merged files
      (hmr @ Self::Hmr { .. }, Self::HmrRebuild { changed_files: other_files }) => {
        let Self::Hmr { changed_files } = hmr else { unreachable!() };
        changed_files.extend(other_files);
        *hmr = Self::HmrRebuild { changed_files: std::mem::take(changed_files) };
      }
      // All other combinations should have been filtered by is_mergeable_with
      _ => {
        eprintln!("Debug: Attempted to merge incompatible tasks. This should be unreachable.");
      }
    }
  }
}
