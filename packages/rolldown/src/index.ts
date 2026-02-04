import './setup';
import { build, type BuildOptions } from './api/build';
import { rolldown } from './api/rolldown';
import type { RolldownBuild } from './api/rolldown/rolldown-build';
import { watch } from './api/watch';
import type {
  RolldownWatcher,
  RolldownWatcherEvent,
  RolldownWatcherWatcherEventMap,
} from './api/watch/watch-emitter';
import type { PreRenderedChunk } from './binding.cjs';
import type { LoggingFunction, WarningHandlerWithDefault } from './log/log-handler';
import type {
  LogLevel,
  LogLevelOption,
  LogOrStringHandler,
  RolldownError,
  RolldownLog,
  RolldownLogWithString,
} from './log/logging';
import type { ChecksOptions } from './options/generated/checks-options';
import type {
  ExternalOptionFunction,
  ExternalOption,
  InputOption,
  InputOptions,
  ModuleTypes,
  OptimizationOptions,
  WatcherOptions,
} from './options/input-options';
import type { TransformOptions } from './options/transform-options';
import type { NormalizedInputOptions } from './options/normalized-input-options';
import type {
  InternalModuleFormat,
  NormalizedOutputOptions,
} from './options/normalized-output-options';
import type {
  CodeSplittingGroup,
  CodeSplittingOptions,
  AddonFunction,
  ChunkFileNamesFunction,
  ChunkingContext,
  CodeSplittingNameFunction,
  AdvancedChunksGroup,
  AdvancedChunksOptions,
  GeneratedCodeOptions,
  GeneratedCodePreset,
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
  BufferEncoding,
  RolldownDirectoryEntry,
  RolldownFileStats,
  RolldownFsModule,
} from './plugin/fs';
import type { GeneralHookFilter, HookFilter, ModuleTypeFilter } from './plugin/hook-filter';
import type { MinimalPluginContext, PluginContextMeta } from './plugin/minimal-plugin-context';
import type { DefineParallelPluginResult } from './plugin/parallel-plugin';
import type {
  EmittedAsset,
  EmittedChunk,
  EmittedFile,
  EmittedPrebuiltChunk,
  GetModuleInfo,
  PluginContextResolveOptions,
  PluginContext,
} from './plugin/plugin-context';
import type { TransformPluginContext } from './plugin/transform-plugin-context';
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
import {
  defineConfig,
  type ConfigExport,
  type RolldownOptionsFunction,
} from './utils/define-config';

export { RUNTIME_MODULE_ID, VERSION } from './constants';
export { build, defineConfig, rolldown, watch };
export { BindingMagicString } from './binding.cjs';
export type {
  AddonFunction,
  CodeSplittingGroup,
  CodeSplittingOptions,
  AsyncPluginHooks,
  AdvancedChunksGroup,
  AdvancedChunksOptions,
  BufferEncoding,
  BuildOptions,
  ChecksOptions,
  ChunkFileNamesFunction,
  ChunkingContext,
  CodeSplittingNameFunction,
  ConfigExport,
  CustomPluginOptions,
  DefineParallelPluginResult,
  EmittedAsset,
  EmittedChunk,
  EmittedFile,
  EmittedPrebuiltChunk,
  ExistingRawSourceMap,
  ExternalOptionFunction,
  ExternalOption,
  FunctionPluginHooks,
  GeneralHookFilter,
  GeneratedCodeOptions,
  GeneratedCodePreset,
  GetModuleInfo,
  GlobalsFunction,
  TransformOptions,
  HookFilter,
  HookFilterExtension,
  ImportKind,
  InputOption,
  InputOptions,
  InternalModuleFormat,
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
  ModuleTypes,
  NormalizedInputOptions,
  NormalizedOutputOptions,
  ObjectHook,
  OptimizationOptions,
  OutputAsset,
  OutputBundle,
  OutputChunk,
  OutputOptions,
  ParallelPluginHooks,
  PartialNull,
  PartialResolvedId,
  Plugin,
  PluginContextResolveOptions,
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
  RolldownDirectoryEntry,
  RolldownFileStats,
  RolldownFsModule,
  RolldownOptions,
  RolldownOptionsFunction,
  RolldownOutput,
  RolldownPlugin,
  RolldownPluginOption,
  RolldownWatcher,
  RolldownWatcherEvent,
  RolldownWatcherWatcherEventMap,
  RolldownError,
  RolldownError as RollupError,
  RolldownLog,
  RolldownLog as RollupLog,
  RolldownLogWithString,
  RolldownLogWithString as RollupLogWithString,
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
