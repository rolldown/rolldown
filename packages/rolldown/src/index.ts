import {
  RolldownOutput,
  RolldownOutputAsset,
  RolldownOutputChunk,
  SourceMap,
} from './types/rolldown-output'
import type {
  ExternalOption,
  InputOption,
  InputOptions,
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
  ObjectHook,
  ParallelPluginHooks,
  PartialResolvedId,
  Plugin,
  ResolveIdResult,
  ResolvedId,
  SourceDescription,
  TransformResult,
} from './plugin'
import { defineParallelPlugin, DefineParallelPluginResult } from './plugin'
import { defineConfig } from './utils/define-config'
import { rolldown, experimental_scan } from './rolldown'
import { ConfigExport } from './types/config-export'
import { BuiltinWasmPlugin } from './plugin/bindingify-builtin-plugin'
import { RolldownBuild } from './rolldown-build'
import { InternalModuleFormat } from './options/bindingify-output-options'
import {
  EmittedAsset,
  EmittedFile,
  PluginContext,
} from './plugin/plugin-context'
import { TransformPluginContext } from './plugin/transfrom-plugin-context'
import { NormalizedOutputOptions } from './options/normalized-output-options'
import { RenderedChunk } from './binding'
import { PartialNull } from './types/utils'
import { NormalizedInputOptions } from './options/normalized-input-options'
import { ModuleInfo } from './types/module-info'
import { MinimalPluginContext } from './log/logger'
import { ExistingRawSourceMap, SourceMapInput } from './types/sourcemap'
import { OutputBundle } from './types/output-bundle'

export {
  defineConfig,
  defineParallelPlugin,
  rolldown,
  experimental_scan,
  BuiltinWasmPlugin,
}

export type {
  RolldownOutputChunk,
  RolldownOptions,
  RolldownOutput,
  RolldownBuild,
  InputOptions,
  NormalizedInputOptions,
  OutputOptions,
  NormalizedOutputOptions,
  Plugin,
  DefineParallelPluginResult,
  ConfigExport,
  ImportKind,
  InputOption,
  ExternalOption,
  ModuleFormat,
  InternalModuleFormat,
  LoadResult,
  TransformResult,
  ResolveIdResult,
  PluginContext,
  TransformPluginContext,
  ObjectHook,
  RenderedChunk,
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
}

// Exports for compatibility

export type {
  RolldownOutput as RollupOutput,
  RolldownOptions as RollupOptions,
  RolldownBuild as RollupBuild,
  RolldownOutputChunk as OutputChunk,
  RolldownOutputAsset as OutputAsset,
}
export type { RollupError, RollupLog, LoggingFunction } from './rollup'
