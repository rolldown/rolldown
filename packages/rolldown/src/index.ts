import {
  RolldownOutput,
  OutputAsset,
  OutputChunk,
  RenderedChunk,
  SourceMap,
} from './types/rolldown-output'
import type {
  InputOptions,
  InputOption,
  ExternalOption,
  JsxOptions,
} from './options/input-options'
import type { ModuleFormat, OutputOptions } from './options/output-options'
import type { RolldownOptions } from './types/rolldown-options'
import type {
  AsyncPluginHooks,
  CustomPluginOptions,
  FunctionPluginHooks,
  ImportKind,
  LoadResult,
  ModuleOptions,
  ModuleType,
  ObjectHook,
  ParallelPluginHooks,
  PartialResolvedId,
  Plugin,
  RolldownPlugin,
  ResolveIdResult,
  ResolvedId,
  SourceDescription,
  TransformResult,
} from './plugin'
import { DefineParallelPluginResult } from './plugin/parallel-plugin'
import { defineConfig } from './utils/define-config'
import { rolldown } from './api/rolldown'
import { watch } from './api/watch'
import { ConfigExport } from './types/config-export'
import { RolldownBuild } from './api/rolldown/rolldown-build'
import {
  EmittedAsset,
  EmittedFile,
  PluginContext,
} from './plugin/plugin-context'
import { TransformPluginContext } from './plugin/transform-plugin-context'
import {
  InternalModuleFormat,
  NormalizedOutputOptions,
} from './options/normalized-output-options'
import { PreRenderedChunk } from './binding'
import { PartialNull } from './types/utils'
import { NormalizedInputOptions } from './options/normalized-input-options'
import { ModuleInfo } from './types/module-info'
import { MinimalPluginContext } from './plugin/minimal-plugin-context'
import { ExistingRawSourceMap, SourceMapInput } from './types/sourcemap'
import { OutputBundle } from './types/output-bundle'
import { version } from '../package.json'
import { WatchOptions } from './options/watch-options'
import { RolldownWatcher } from './api/watch/watch-emitter'
import { build, type BuildOptions } from './api/build'

export { defineConfig, rolldown, watch, build }
export const VERSION: string = version

export type {
  OutputAsset,
  OutputChunk,
  RolldownOptions,
  RolldownOutput,
  RolldownBuild,
  InputOptions,
  NormalizedInputOptions,
  OutputOptions,
  NormalizedOutputOptions,
  Plugin,
  RolldownPlugin,
  DefineParallelPluginResult,
  ConfigExport,
  ImportKind,
  InputOption,
  ExternalOption,
  ModuleFormat,
  ModuleType,
  InternalModuleFormat,
  LoadResult,
  TransformResult,
  ResolveIdResult,
  PluginContext,
  TransformPluginContext,
  ObjectHook,
  PreRenderedChunk,
  SourceMap,
  SourceDescription,
  PartialNull,
  PartialResolvedId,
  ResolvedId,
  ModuleOptions,
  ModuleInfo,
  MinimalPluginContext,
  EmittedFile,
  EmittedAsset,
  CustomPluginOptions,
  AsyncPluginHooks,
  ParallelPluginHooks,
  FunctionPluginHooks,
  ExistingRawSourceMap,
  SourceMapInput,
  OutputBundle,
  JsxOptions,
  WatchOptions,
  RolldownWatcher,
  BuildOptions,
  RenderedChunk,
}

export type { RollupError, RollupLog, LoggingFunction } from './types/misc'
