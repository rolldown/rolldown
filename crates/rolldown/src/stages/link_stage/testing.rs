//! Testing-only access to the production link boundary.

use std::{
  cell::RefCell,
  hint::black_box,
  sync::{Arc, Mutex, PoisonError},
  time::{Duration, Instant},
};

use rolldown_common::ScanMode;
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_fs::FileSystem;

use crate::{Bundle, BundleOutput};

use super::{LinkStage, LinkStageOutput};

type LinkObserver = Box<dyn FnOnce(&mut LinkStageOutput) + Send + 'static>;

tokio::task_local! {
  static LINK_BASELINE_OBSERVER: RefCell<Option<LinkObserver>>;
}

/// Result of one scan-then-link baseline sample.
pub struct LinkBaselineSample {
  /// Wall-clock time for `LinkStage::new(...).link()`.
  pub elapsed: Duration,
  /// Number of modules present at the measured link boundary, including runtime modules.
  pub linked_module_count: usize,
}

/// Scans outside the timer, then measures the same link boundary as production.
///
/// All three link outputs stay alive until after the clock is stopped. The
/// function intentionally exposes neither scan nor link implementation types.
pub async fn run_link_baseline_once<Fs>(mut bundle: Bundle<Fs>) -> BuildResult<LinkBaselineSample>
where
  Fs: FileSystem + Clone + 'static,
{
  let scan_stage_output = bundle.scan_modules(ScanMode::Full).await?;

  let started = Instant::now();
  let (link_stage_output, ast_table, used_symbol_refs) =
    LinkStage::new(scan_stage_output, &bundle.options).link();
  let elapsed = started.elapsed();
  let _ = black_box((&link_stage_output, &ast_table, &used_symbol_refs));
  let linked_module_count = link_stage_output.module_table.len();

  drop((link_stage_output, ast_table, used_symbol_refs));

  Ok(LinkBaselineSample { elapsed, linked_module_count })
}

/// Runs the standard Generate path while observing link diagnostics by shared borrow.
///
/// The diagnostic accumulator is moved out and restored around the observer because
/// it deliberately exposes no general read API. Generate receives the same ordered
/// diagnostics immediately afterward.
pub async fn run_generate_with_link_baseline_observer<Fs, T, O>(
  bundle: Bundle<Fs>,
  observer: O,
) -> (BuildResult<BundleOutput>, Option<T>)
where
  Fs: FileSystem + Clone + 'static,
  T: Send + 'static,
  O: FnOnce(&[BuildDiagnostic], usize) -> T + Send + 'static,
{
  let observation = Arc::new(Mutex::new(None));
  let observation_for_callback = Arc::clone(&observation);
  let callback: LinkObserver = Box::new(move |link_stage_output| {
    let diagnostics =
      std::mem::take(&mut link_stage_output.diagnostics).into_iter().collect::<Vec<_>>();
    let value = observer(&diagnostics, link_stage_output.module_table.len());
    link_stage_output.diagnostics = diagnostics.into();
    *observation_for_callback.lock().unwrap_or_else(PoisonError::into_inner) = Some(value);
  });
  let result = LINK_BASELINE_OBSERVER.scope(RefCell::new(Some(callback)), bundle.generate()).await;
  let observation = observation.lock().unwrap_or_else(PoisonError::into_inner).take();
  (result, observation)
}

pub(super) fn observe_link_output(output: &mut LinkStageOutput) {
  let _ = LINK_BASELINE_OBSERVER.try_with(|observer| {
    if let Some(observer) = observer.borrow_mut().take() {
      observer(output);
    }
  });
}
