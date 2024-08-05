pub mod hook_metric;

#[derive(Debug)]
pub struct Stats {
  pub hook_metric: Vec<hook_metric::HookMetric>,
}
