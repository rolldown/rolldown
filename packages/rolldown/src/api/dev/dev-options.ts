import type { BindingClientHmrUpdate } from '../../binding';
import type { RolldownOutput } from '../../types/rolldown-output';

type DevOnHmrUpdates = (
  result: Error | {
    updates: BindingClientHmrUpdate[];
    changedFiles: string[];
  },
) => void | Promise<void>;

type DevOnOutput = (
  result: Error | RolldownOutput,
) => void | Promise<void>;

export interface DevWatchOptions {
  /**
   * If `true`, files are not written to disk.
   * @default false
   */
  skipWrite?: boolean;
  /**
   * If `true`, use polling instead of native file system events for watching.
   * @default false
   */
  usePolling?: boolean;
  /**
   * Poll interval in milliseconds (only used when usePolling is true).
   * @default 100
   */
  pollInterval?: number;
  /**
   * If `true`, use debounced watcher. If `false`, use non-debounced watcher for immediate responses.
   * @default true
   */
  useDebounce?: boolean;
  /**
   * Debounce duration in milliseconds (only used when useDebounce is true).
   * @default 10
   */
  debounceDuration?: number;
  /**
   * Whether to compare file contents for poll-based watchers (only used when usePolling is true).
   * When enabled, poll watchers will check file contents to determine if they actually changed.
   * @default false
   */
  compareContentsForPolling?: boolean;
  /**
   * Tick rate in milliseconds for debounced watchers (only used when useDebounce is true).
   * Controls how frequently the debouncer checks for events to process.
   * When not specified, the debouncer will auto-select an appropriate tick rate (1/4 of the debounce duration).
   * @default undefined (auto-select)
   */
  debounceTickRate?: number;
}

export interface DevOptions {
  onHmrUpdates?: DevOnHmrUpdates;
  onOutput?: DevOnOutput;
  /**
   * Strategy for triggering rebuilds after HMR updates.
   * - `'always'`: Always trigger a rebuild after HMR updates
   * - `'auto'`: Trigger rebuild only if HMR updates contain full reload updates
   * - `'never'`: Never trigger rebuild after HMR updates (default)
   * @default 'auto'
   */
  rebuildStrategy?: 'always' | 'auto' | 'never';
  watch?: DevWatchOptions;
}
