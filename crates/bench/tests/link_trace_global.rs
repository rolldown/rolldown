use std::{
  sync::atomic::{AtomicBool, Ordering},
  time::Duration,
};

use bench::link_baseline::trace::{LinkTraceLayer, records_link_trace_metadata};
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
}
