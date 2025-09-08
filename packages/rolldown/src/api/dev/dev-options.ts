import type { BindingHmrUpdate } from '../../binding';

export interface DevWatchOptions {
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
   * @default 100
   */
  debounceDuration?: number;
}

export interface DevOptions {
  onHmrUpdates?: (updates: BindingHmrUpdate[]) => void | Promise<void>;
  watch?: DevWatchOptions;
}
