import type { WatchOptions } from '../../options/watch-options';
import { type RolldownWatcher, WatcherEmitter } from './watch-emitter';
import { createWatcher } from './watcher';

// Compat to `rollup.watch`
// NOTE: This must be a function declaration (not const arrow function) for TypeDoc
// to correctly associate JSDoc comments with modifier tags like @experimental.
/**
 * The API compatible with Rollup's `watch` function.
 *
 * This function will rebuild the bundle when it detects that the individual modules have changed on disk.
 *
 * Note that when using this function, it is your responsibility to call `event.result.close()` in response to the `BUNDLE_END` event to avoid resource leaks.
 *
 * @param input The watch options object or the list of them.
 * @returns A watcher object.
 *
 * @example
 * ```js
 * import { watch } from 'rolldown';
 *
 * const watcher = watch({ /* ... *\/ });
 * watcher.on('event', (event) => {
 *   if (event.code === 'BUNDLE_END') {
 *     console.log(event.duration);
 *     event.result.close();
 *   }
 * });
 *
 * // Stop watching
 * watcher.close();
 * ```
 *
 * @experimental
 * @category Programmatic APIs
 */
export function watch(input: WatchOptions | WatchOptions[]): RolldownWatcher {
  const emitter = new WatcherEmitter();
  createWatcher(emitter, input);
  return emitter;
}
