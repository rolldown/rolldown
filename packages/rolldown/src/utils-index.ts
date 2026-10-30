export { parse, parseSync, type ParseResult, type ParserOptions } from './utils/parse';
export type * as ESTree from '@oxc-project/types';
export { minify, type MinifyOptions, type MinifyResult, minifySync } from './utils/minify';
export {
  chokidar,
  FSWatcher,
  watch,
  Watcher,
  type ChokidarOptions,
  type Ignored,
  type NativeWatchEvent,
  type WatchEventName,
  type WatcherEvent,
  type WatcherEventName,
  type WatcherOptions,
  type WatchMatcher,
} from './utils/notify';
export {
  transform,
  type TransformOptions,
  type TransformResult,
  transformSync,
  type TsconfigRawOptions,
  type TsconfigCompilerOptions,
} from './utils/transform';
export { TsconfigCache } from './utils/resolve-tsconfig';
export { Visitor, type VisitorObject } from './utils/visitor';
