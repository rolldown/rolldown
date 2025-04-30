import { version } from '../package.json';
import { build, type BuildOptions } from './api/build';
import { rolldown } from './api/rolldown';
import { RolldownBuild } from './api/rolldown/rolldown-build';
import { watch } from './api/watch';
import type {
  RolldownWatcher,
  RolldownWatcherEvent,
} from './api/watch/watch-emitter';
import type { PreRenderedChunk } from './binding';
import type {
  LoggingFunction,
  WarningHandlerWithDefault,
} from './log/log-handler';
import type {
  LogLevel,
  LogLevelOption,
  LogOrStringHandler,
  RollupError,
  RollupLog,
  RollupLogWithString,
} from './log/logging';
import type {
  ExternalOption,
  InputOption,
  InputOptions,
  JsxOptions,
  WatcherOptions,
} from './options/input-options';
import type { NormalizedInputOptions } from './options/normalized-input-options';
import type {
  InternalModuleFormat,
  NormalizedOutputOptions,
} from './options/normalized-output-options';
import type {
  AddonFunction,
  ChunkFileNamesFunction,
  GlobalsFunction,
  MinifyOptions,
  ModuleFormat,
  OutputOptions,
  PreRenderedAsset,
} from './options/output-options';
import type { WatchOptions } from './options/watch-options';
import type {
  AsyncPluginHooks,
  CustomPluginOptions,
  FunctionPluginHooks,
  HookFilterExtension,
  ImportKind,
  LoadResult,
  ModuleOptions,
  ModuleType,
  ObjectHook,
  ParallelPluginHooks,
  PartialResolvedId,
  Plugin,
  ResolvedId,
  ResolveIdExtraOptions,
  ResolveIdResult,
  RolldownPlugin,
  RolldownPluginOption,
  SourceDescription,
  TransformResult,
} from './plugin';
import type {
  GeneralHookFilter,
  HookFilter,
  ModuleTypeFilter,
} from './plugin/hook-filter';
import type {
  MinimalPluginContext,
  PluginContextMeta,
} from './plugin/minimal-plugin-context';
import type { DefineParallelPluginResult } from './plugin/parallel-plugin';
import type {
  EmittedAsset,
  EmittedFile,
  GetModuleInfo,
  PluginContext,
} from './plugin/plugin-context';
import type { TransformPluginContext } from './plugin/transform-plugin-context';
import type { ConfigExport } from './types/config-export';
import type { SourcemapIgnoreListOption } from './types/misc';
import type { ModuleInfo } from './types/module-info';
import type { TreeshakingOptions } from './types/module-side-effects';
import type { OutputBundle } from './types/output-bundle';
import type { RolldownOptions } from './types/rolldown-options';
import type {
  OutputAsset,
  OutputChunk,
  RenderedChunk,
  RenderedModule,
  RolldownOutput,
  SourceMap,
} from './types/rolldown-output';
import type { ExistingRawSourceMap, SourceMapInput } from './types/sourcemap';
import type { PartialNull } from './types/utils';
import { defineConfig } from './utils/define-config';

export { build, defineConfig, rolldown, watch };
export const VERSION: string = version;

export type {
  AddonFunction,
  AsyncPluginHooks,
  BuildOptions,
  ChunkFileNamesFunction,
  ConfigExport,
  CustomPluginOptions,
  DefineParallelPluginResult,
  EmittedAsset,
  EmittedFile,
  ExistingRawSourceMap,
  ExternalOption,
  FunctionPluginHooks,
  GeneralHookFilter,
  GetModuleInfo,
  GlobalsFunction,
  HookFilter,
  HookFilterExtension,
  ImportKind,
  InputOption,
  InputOptions,
  InternalModuleFormat,
  JsxOptions,
  LoadResult,
  LoggingFunction,
  LogLevel,
  LogLevelOption,
  LogOrStringHandler,
  MinifyOptions,
  MinimalPluginContext,
  ModuleFormat,
  ModuleInfo,
  ModuleOptions,
  ModuleType,
  ModuleTypeFilter,
  NormalizedInputOptions,
  NormalizedOutputOptions,
  ObjectHook,
  OutputAsset,
  OutputBundle,
  OutputChunk,
  OutputOptions,
  ParallelPluginHooks,
  PartialNull,
  PartialResolvedId,
  Plugin,
  PluginContext,
  PluginContextMeta,
  PreRenderedAsset,
  PreRenderedChunk,
  RenderedChunk,
  RenderedModule,
  ResolvedId,
  ResolveIdExtraOptions,
  ResolveIdResult,
  RolldownBuild,
  RolldownOptions,
  RolldownOutput,
  RolldownPlugin,
  RolldownPluginOption,
  RolldownWatcher,
  RolldownWatcherEvent,
  RollupError,
  RollupLog,
  RollupLogWithString,
  SourceDescription,
  SourceMap,
  SourcemapIgnoreListOption,
  SourceMapInput,
  TransformPluginContext,
  TransformResult,
  TreeshakingOptions,
  WarningHandlerWithDefault,
  WatcherOptions,
  WatchOptions,
};
