use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

#[derive(Default, Debug, Clone)]
pub struct HookMetric {
  transform: Arc<AtomicU64>,
  resolve: Arc<AtomicU64>,
  load: Arc<AtomicU64>,
  pub name: String,
}

#[derive(Debug)]
pub struct MsMetric {
  transform: f64,
  resolve: f64,
  load: f64,
  pub name: String,
}

pub enum MetricType {
  Transform,
  Resolve,
  Load,
}

impl HookMetric {
  pub fn guard(&self, ty: MetricType) -> Guard {
    let counter = match ty {
      MetricType::Transform => &self.transform,
      MetricType::Resolve => &self.resolve,
      MetricType::Load => &self.load,
    };
    Guard { start: Instant::now(), counter: Arc::clone(counter) }
  }

  /// Convert the metric to milliseconds based
  pub fn to_ms(self) -> MsMetric {
    MsMetric {
      transform: self.transform.load(Ordering::Relaxed) as f64 / 1000f64,
      resolve: self.resolve.load(Ordering::Relaxed) as f64 / 1000f64,
      load: self.resolve.load(Ordering::Relaxed) as f64 / 1000f64,
      name: self.name,
    }
  }
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
