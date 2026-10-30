import { createWatcherApi, type WatcherApi } from '@rolldown/notify/factory';
import type * as Notify from '@rolldown/notify/factory';
import { createChokidarApi, type ChokidarApi } from '@rolldown/notify/factory/chokidar';
import type * as Chokidar from '@rolldown/notify/factory/chokidar';
import * as binding from '../binding.cjs';

/**
 * @category Utilities
 * @experimental
 */
export const chokidar: ChokidarApi = /* #__PURE__ */ createChokidarApi(binding);

/**
 * @category Utilities
 * @experimental
 */
export type ChokidarOptions = Chokidar.WatcherOptions;

/**
 * @category Utilities
 * @experimental
 */
export const FSWatcher: ChokidarApi['FSWatcher'] = /* #__PURE__ */ (() => chokidar.FSWatcher)();

/**
 * @category Utilities
 * @experimental
 */
export type FSWatcher = Chokidar.FSWatcher;

/**
 * @category Utilities
 * @experimental
 */
export type NativeWatchEvent = Chokidar.NativeWatchEvent;

/**
 * @category Utilities
 * @experimental
 */
export type WatchEventName = Chokidar.WatchEventName;

const api = /* #__PURE__ */ createWatcherApi(binding);

/**
 * @category Utilities
 * @experimental
 */
export const Watcher: WatcherApi['Watcher'] = /* #__PURE__ */ (() => api.Watcher)();

/**
 * @category Utilities
 * @experimental
 */
export const watch: WatcherApi['watch'] = /* #__PURE__ */ (() => api.watch)();

/**
 * @category Utilities
 * @experimental
 */
export type Watcher = Notify.Watcher;

/**
 * @category Utilities
 * @experimental
 */
export type Ignored = Notify.Ignored;

/**
 * @category Utilities
 * @experimental
 */
export type WatcherEvent = Notify.WatcherEvent;

/**
 * @category Utilities
 * @experimental
 */
export type WatcherEventName = Notify.WatcherEventName;

/**
 * @category Utilities
 * @experimental
 */
export type WatcherOptions = Notify.WatcherOptions;

/**
 * @category Utilities
 * @experimental
 */
export type WatchMatcher = Notify.WatchMatcher;
