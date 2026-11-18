use std::hint::black_box;

use rolldown_tracking_allocator::{TrackingAllocator, reset, stats};

#[global_allocator]
static ALLOC: TrackingAllocator = TrackingAllocator;

const SIZE: usize = 1_000_000;

/// Harness threads flush batched deltas at any point, and a thread that exits
/// with unflushed pending skews the balance by up to one batch. Allow a few
/// batches of drift around every exact expectation.
const SLACK: usize = 256 * 1024;

/// Stays below the flush threshold on purpose, so only the exit flush can
/// record it.
const CHUNK: usize = 48 * 1024;

// A single test fn: the counters are process-global, so parallel test fns
// would see each other's allocations.
#[test]
fn tracks_allocations() {
  // Hold ballast so the global balance stays positive for the whole test.
  // stats() clamps a negative balance to zero, and near zero that clamp makes
  // the deltas below meaningless. Observed on Linux CI, where a harness thread
  // exits with unflushed pending before the test body runs.
  let ballast = black_box(vec![1u8; 2 * SIZE]);

  reset();
  let before = stats();

  let v = black_box(vec![0u8; SIZE]);
  let during = stats();
  assert!(during.live_bytes + SLACK >= before.live_bytes + SIZE);
  assert!(during.peak_bytes + SLACK >= before.live_bytes + SIZE);
  assert!(during.alloc_count >= 1);

  drop(black_box(v));
  let after = stats();
  assert!(after.live_bytes <= during.live_bytes - SIZE + SLACK);
  // The peak keeps the high-water mark after the free.
  assert!(after.peak_bytes + SLACK >= before.live_bytes + SIZE);

  // A sub-batch allocation on a thread that exits, freed on this thread, must
  // not skew the balance: the exit flush records what the retired thread
  // never flushed itself. Rolldown does this in normal operation, through
  // expiring spawn_blocking threads and a short-lived sourcemap thread.
  let base = stats();
  for _ in 0..32 {
    let buf = std::thread::spawn(|| black_box(vec![3u8; CHUNK])).join().unwrap();
    drop(black_box(buf));
  }
  let churned = stats();
  assert!(churned.live_bytes + SLACK >= base.live_bytes);
  assert!(churned.live_bytes <= base.live_bytes + SLACK);

  reset();
  let fresh = stats();
  assert!(fresh.peak_bytes <= fresh.live_bytes + SLACK);
  // Only concurrent flushes from other threads can raise this after a reset.
  assert!(fresh.alloc_count <= 2048);

  drop(black_box(ballast));
}
