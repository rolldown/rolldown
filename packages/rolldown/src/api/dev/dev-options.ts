import type { BindingHmrUpdate } from '../../binding';

export interface DevWatchOptions {
  usePolling?: boolean;
  pollInterval?: number;
}

export interface DevOptions {
  onHmrUpdates?: (updates: BindingHmrUpdate[]) => void | Promise<void>;
  watch?: DevWatchOptions;
}
