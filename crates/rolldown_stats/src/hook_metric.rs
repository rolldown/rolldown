use std::fmt::Debug;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

#[derive(Default, Debug, Clone)]
pub struct HookMetric {
  transform: Arc<AtomicU64>,
  resolve: Arc<AtomicU64>,
  load: Arc<AtomicU64>,
  transform_ast: Arc<AtomicU64>,
  pub name: String,
}

impl HookMetric {
  pub fn to_ms(self) -> MsMetric {
    MsMetric {
      name: self.name,
      metrics: vec![
        TaggedMsMetric::Transform(self.transform.load(Ordering::Relaxed) as f64 / 1000.0),
        TaggedMsMetric::Resolve(self.resolve.load(Ordering::Relaxed) as f64 / 1000.0),
        TaggedMsMetric::Load(self.load.load(Ordering::Relaxed) as f64 / 1000.0),
        TaggedMsMetric::TransformAst(self.transform_ast.load(Ordering::Relaxed) as f64 / 1000.0),
      ],
    }
  }

  pub fn guard(&self, ty: MetricType) -> Guard {
    let counter = match ty {
      MetricType::Transform => Arc::clone(&self.transform),
      MetricType::Resolve => Arc::clone(&self.resolve),
      MetricType::Load => Arc::clone(&self.load),
      MetricType::TransformAst => Arc::clone(&self.transform_ast),
    };
    Guard { start: Instant::now(), counter }
  }
}

pub struct MsMetric {
  pub name: String,
  metrics: Vec<TaggedMsMetric>,
}

impl MsMetric {
  pub fn total(&self) -> f64 {
    self.metrics.iter().map(|m| m.value()).sum()
  }
  pub fn sort_metrics(&mut self) {
    self.metrics.sort_unstable_by(|a, b| b.value().partial_cmp(&a.value()).unwrap());
  }
}

impl Debug for MsMetric {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    writeln!(f, "{}", self.name)?;
    for metric in self.metrics.iter() {
      writeln!(f, "  |- {:?}", metric)?;
    }
    Ok(())
  }
}

enum TaggedMsMetric {
  Transform(f64),
  TransformAst(f64),
  Resolve(f64),
  Load(f64),
}

impl Debug for TaggedMsMetric {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      TaggedMsMetric::Transform(v) => write!(f, "transform: {:.2}ms", *v),
      TaggedMsMetric::Resolve(v) => write!(f, "resolve: {:.2}ms", *v),
      TaggedMsMetric::Load(v) => write!(f, "load: {:.2}ms", *v),
      TaggedMsMetric::TransformAst(v) => write!(f, "transform_ast: {:.2}ms", *v),
    }
  }
}

impl TaggedMsMetric {
  pub fn value(&self) -> f64 {
    match self {
      TaggedMsMetric::Transform(ms)
      | TaggedMsMetric::Resolve(ms)
      | TaggedMsMetric::Load(ms)
      | TaggedMsMetric::TransformAst(ms) => *ms,
    }
  }
}

pub enum MetricType {
  Transform,
  TransformAst,
  Resolve,
  Load,
}

pub struct Guard {
  start: Instant,
  counter: Arc<AtomicU64>,
}
impl Drop for Guard {
  fn drop(&mut self) {
    let duration = self.start.elapsed().as_micros();
    self.counter.fetch_add(duration as u64, Ordering::SeqCst);
  }
}

pub fn print_metrics(mut metrics: Vec<MsMetric>) {
  /// String repr of metric will be helpful for pretty print
  metrics.sort_unstable_by(|a, b| b.total().total_cmp(&a.total()));
  for metric in metrics.iter_mut() {
    metric.sort_metrics();
  }
  for metric in metrics.iter() {
    print!("{:?}", metric);
  }
}
