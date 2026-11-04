import type { BindingClientHmrUpdate } from '../../binding.cjs';
import type { WatcherOptions } from '../../options/input-options';
import type { RolldownOutput } from '../../types/rolldown-output';

type DevOnHmrUpdates = (
  result:
    | Error
    | {
        updates: BindingClientHmrUpdate[];
        changedFiles: string[];
      },
) => void | Promise<void>;

type DevOnOutput = (result: Error | RolldownOutput) => void | Promise<void>;

type DevOnAdditionalAssets = (output: RolldownOutput) => void | Promise<void>;

/**
 * Watch options supported by the dev engine.
 * The dev watcher uses debounced events by default (`watcher.useDebounce: true`).
 */
export interface DevWatchOptions extends Pick<
  WatcherOptions,
  'skipWrite' | 'watcher' | 'include' | 'exclude'
> {
  /**
   * If `false`, no file system watcher is started, so file changes are never
   * picked up and no rebuild or HMR update is triggered on their own.
   * @default true
   */
  enabled?: boolean;
}

export interface DevOptions {
  onHmrUpdates?: DevOnHmrUpdates;
  onOutput?: DevOnOutput;
  /**
   * Called with assets emitted while generating an HMR patch or compiling a
   * lazy entry (e.g. an image newly imported by the changed/lazy module).
   *
   * These never go through {@link onOutput}, so a consumer that serves built
   * files (e.g. Vite's bundled dev server) must register this to receive them
   * and write them to its in-memory file store before the client requests them.
   */
  onAdditionalAssets?: DevOnAdditionalAssets;
  /**
   * Strategy for triggering rebuilds after HMR updates.
   * - `'always'`: Always trigger a rebuild after HMR updates
   * - `'never'`: Never trigger rebuild after HMR updates. The server no longer
   *   decides full reloads, so there is no `'auto'` upgrade anymore; pull fresh
   *   bundle output explicitly (e.g. `ensureLatestBuildOutput`) when needed.
   * @default 'never'
   */
  rebuildStrategy?: 'always' | 'never';
  /** File watching options. */
  watch?: DevWatchOptions;
  /**
   * Whether the `hotUpdate` plugin hook is called. `experimental.devMode.hotUpdate` also turns it on.
   * @default false
   */
  hotUpdate?: boolean;
}
