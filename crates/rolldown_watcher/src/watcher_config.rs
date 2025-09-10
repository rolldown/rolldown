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

  /// Whether to compare file contents for poll-based watchers.
  /// When enabled, poll watchers will check file contents to determine if they actually changed.
  /// Default to false.
  ///
  /// ⚠️Only take effect for poll-based watchers.
  pub compare_contents_for_polling: bool,

  /// Tick rate for debounced watchers (in milliseconds).
  /// Controls how frequently the debouncer checks for events to process.
  /// When None, the debouncer will auto-select an appropriate tick rate (1/4 of the debounce duration).
  ///
  /// ⚠️Only take effect for debounced watchers.
  pub debounce_tick_rate: Option<u64>,
}

impl Default for WatcherConfig {
  fn default() -> Self {
    Self {
      debounce_delay: 10,
      // Chokidar's default poll interval is 100ms
      poll_interval: 100,
      compare_contents_for_polling: false,
      debounce_tick_rate: None,
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

  pub fn debounce_tick_rate(&self) -> Option<Duration> {
    self.debounce_tick_rate.map(Duration::from_millis)
  }

  pub fn to_notify_config(&self) -> notify::Config {
    notify::Config::default()
      .with_poll_interval(self.poll_interval_duration())
      .with_compare_contents(self.compare_contents_for_polling)
  }
}
