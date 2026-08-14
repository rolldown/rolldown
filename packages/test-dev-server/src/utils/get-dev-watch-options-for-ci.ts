import type { DevWatchOptions } from 'rolldown/experimental';

export function getDevWatchOptionsForCi() {
  return {
    usePolling: true,
    pollInterval: 50,
    // The poll watcher stores mtime at whole-second granularity and only
    // emits an mtime-based change when the second advances; with content
    // comparison off, two writes to the same file within the same second are
    // invisible. On loaded CI runners that blind spot drops the recovery edit
    // in the dev-server specs and the build never re-fires. Comparing content
    // hashes adds a second detection path for same-second rewrites.
    compareContentsForPolling: true,
    useDebounce: true,
    debounceDuration: 310,
    debounceTickRate: 300,
  } satisfies DevWatchOptions;
}
