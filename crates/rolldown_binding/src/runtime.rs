use std::num::NonZeroUsize;

pub fn worker_threads() -> usize {
  // Calling Builder::worker_threads overrides Tokio's environment handling, so honor its
  // setting explicitly. The Rolldown-specific setting takes precedence.
  ["ROLLDOWN_WORKER_THREADS", "TOKIO_WORKER_THREADS"]
    .into_iter()
    .find_map(|name| std::env::var(name).ok().and_then(|value| parse_thread_count(&value)))
    .unwrap_or_else(|| {
      let physical = num_cpus::get_physical();
      // On Linux get_physical() can report the host's cores, ignoring both cgroup quotas
      // and CPU affinity. available_parallelism() accounts for these restrictions.
      let available = std::thread::available_parallelism().map_or(physical, NonZeroUsize::get);
      default_worker_threads(physical, available)
    })
}

fn parse_thread_count(value: &str) -> Option<usize> {
  value.parse::<NonZeroUsize>().ok().map(NonZeroUsize::get)
}

fn default_worker_threads(physical: usize, available: usize) -> usize {
  // Keep the existing 1.5x policy: module tasks also do blocking work on Tokio workers.
  // Apply it to the CPUs this process can use, rather than unrestricted host topology.
  physical.min(available) * 3 / 2
}

#[cfg(test)]
mod tests {
  use super::{default_worker_threads, parse_thread_count};

  #[test]
  fn respects_cpu_limits() {
    assert_eq!(default_worker_threads(64, 2), 3);
    assert_eq!(default_worker_threads(64, 1), 1);
  }

  #[test]
  fn preserves_unrestricted_defaults() {
    assert_eq!(default_worker_threads(16, 32), 24);
    assert_eq!(default_worker_threads(16, 16), 24);
    assert_eq!(default_worker_threads(1, 1), 1);
  }

  #[test]
  fn rejects_invalid_thread_counts() {
    for value in ["", "0", "-1", "invalid", "1.5", "999999999999999999999999999999"] {
      assert_eq!(parse_thread_count(value), None, "{value}");
    }
    assert_eq!(parse_thread_count("1"), Some(1));
    assert_eq!(parse_thread_count("2"), Some(2));
  }
}
