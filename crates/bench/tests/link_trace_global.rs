use std::{
  sync::atomic::{AtomicBool, Ordering},
  time::Duration,
};

use bench::link_baseline::trace::{LinkTraceLayer, records_link_trace_metadata};
use bench::link_baseline::{
  BaselineConfig, BaselineMode, run_baseline_with_trace_layer,
  workloads::{DEFAULT_SEED, synthetic_workload_with_seed},
};
use rayon::ThreadPoolBuilder;
use tracing_subscriber::{Layer as _, filter::filter_fn, prelude::*};

#[test]
fn global_collector_reaches_existing_rayon_workers_but_context_does_not() {
  let pool = ThreadPoolBuilder::new().num_threads(4).build().expect("Rayon pool");
  pool.broadcast(|_| {});

  let layer = LinkTraceLayer::default();
  let subscriber = tracing_subscriber::registry()
    .with(layer.clone().with_filter(filter_fn(records_link_trace_metadata)));
  tracing::subscriber::set_global_default(subscriber).expect("fresh integration-test process");

  let trace_field_was_evaluated = AtomicBool::new(false);
  let link = tracing::debug_span!(target: "rolldown::stages::link_stage", "link");
  {
    let _link = link.enter();
    tracing::trace!(
      value = trace_field_was_evaluated.swap(true, Ordering::Relaxed),
      "filtered trace event"
    );
    pool.broadcast(|context| {
      let pass_name = format!("test::ExplicitWorker{}", context.index());
      let pass = tracing::debug_span!(
        target: "rolldown::pass",
        parent: &link,
        "run_pass",
        pass = pass_name.as_str()
      );
      let _pass = pass.enter();
    });
    pool.broadcast(|context| {
      let pass_name = format!("test::ContextualWorker{}", context.index());
      let pass = tracing::debug_span!(
        target: "rolldown::pass",
        "run_pass",
        pass = pass_name.as_str()
      );
      let _pass = pass.enter();
    });
  }
  drop(link);

  assert!(!trace_field_was_evaluated.load(Ordering::Relaxed));
  let sample = layer.sample(Duration::from_secs(1)).expect("link trace");
  let direct_passes =
    sample.direct_children.iter().filter(|span| span.target == "rolldown::pass").count();
  assert_eq!(direct_passes, 4, "global dispatcher must reach every existing Rayon worker");
  assert_eq!(
    sample.detached_passes.len(),
    4,
    "the current parent is thread-local and must not be assumed to propagate"
  );

  layer.reset().expect("completed synthetic trace can reset");
  let workload =
    synthetic_workload_with_seed("overhead-64", DEFAULT_SEED).expect("baseline workload");
  let runtime =
    tokio::runtime::Builder::new_current_thread().enable_all().build().expect("Tokio runtime");
  let report = runtime
    .block_on(run_baseline_with_trace_layer(
      BaselineConfig {
        mode: BaselineMode::LinkTrace,
        warmups: 0,
        samples: 1,
        iterations_per_sample: 1,
        canonical: false,
      },
      workload,
      &layer,
    ))
    .expect("production link trace");
  let production = &report.trace_samples[0];
  let pass_spans = production
    .direct_children
    .iter()
    .filter(|span| span.target == "rolldown::pass")
    .collect::<Vec<_>>();
  let pass_names = pass_spans
    .iter()
    .map(|span| span.pass.as_deref().expect("production pass name"))
    .collect::<Vec<_>>();
  let expected = [
    "ExtractGlobalConstantsPass",
    "CanonicalizeEntriesPass",
    "CollectInitialDependenciesPass",
    "CollectExternalStarExportsPass",
    "ComputeModuleExecutionOrderPass",
    "ComputeTlaPass",
    "DetermineModuleFormatsPass",
    "ComputeCjsNamespaceMergesPass",
    "ComputeDynamicExportsPass",
    "PlanModuleWrappingPass",
    "CreateWrapperDeclarationsPass",
    "NormalizeLazyExportsPass",
    "DetermineModuleSideEffectsPass",
    "CollectResolvedExportsPass",
  ];
  assert_eq!(pass_names.len(), expected.len());
  assert!(pass_names.iter().zip(expected).all(|(pass, suffix)| pass.ends_with(suffix)));
  assert!(production.detached_passes.is_empty());
}
