use std::time::Duration;

#[derive(Debug, Clone)]
pub struct WatcherConfig {
  /// Debounce delay for debounced watchers (in milliseconds).
  /// Default to 10ms.
  ///
  /// ⚠️Only take effect for debounced watchers.
  pub debounce_delay: u64,

  /// Poll interval for poll-based watchers (in milliseconds).
  /// Default to 100ms.
  ///
  /// ⚠️Only take effect for poll-based watchers.
  pub poll_interval: u64,
}

impl Default for WatcherConfig {
  fn default() -> Self {
    Self {
      debounce_delay: 10,
      // Chokidar's default poll interval is 100ms
      poll_interval: 100,
    }
  }
}

impl WatcherConfig {
  pub fn debounce_delay_duration(&self) -> Duration {
    Duration::from_millis(self.debounce_delay)
  }

  pub fn poll_interval_duration(&self) -> Duration {
    Duration::from_millis(self.poll_interval)
  }

  pub fn to_notify_config(&self) -> notify::Config {
    notify::Config::default()
      .with_poll_interval(self.poll_interval_duration())
      .with_compare_contents(false)
  }
}
