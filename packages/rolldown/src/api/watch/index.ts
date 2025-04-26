import type { WatchOptions } from '../../options/watch-options';
import { type RolldownWatcher, WatcherEmitter } from './watch-emitter';
import { createWatcher } from './watcher';

// Compat to `rollup.watch`
export const watch = (
  input: WatchOptions | WatchOptions[],
): RolldownWatcher => {
  const emitter = new WatcherEmitter();
  createWatcher(emitter, input);
  return emitter;
};
