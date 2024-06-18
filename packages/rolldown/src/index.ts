import { RolldownOutput, RolldownOutputChunk } from './types/rolldown-output'
import type {
  ExternalOption,
  InputOption,
  InputOptions,
} from './options/input-options'
import type { ModuleFormat, OutputOptions } from './options/output-options'
import type { RolldownOptions } from './types/rolldown-options'
import type {
  ImportKind,
  LoadResult,
  ObjectHook,
  Plugin,
  ResolveIdResult,
  TransformResult,
} from './plugin'
import { defineParallelPlugin, DefineParallelPluginResult } from './plugin'
import { defineConfig } from './utils/define-config'
import { rolldown, experimental_scan } from './rolldown'
import { ConfigExport } from './types/config-export'
import { BuiltinWasmPlugin } from './plugin/bindingify-builtin-plugin'
import { RolldownBuild } from './rolldown-build'
import { InternalModuleFormat } from './options/bindingify-output-options'
import { PluginContext } from './plugin/plugin-context'
import { TransformPluginContext } from './plugin/transfrom-plugin-context'

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
  OutputOptions,
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
}

// Exports for compatibility

export type {
  RolldownOutput as RollupOutput,
  RolldownOptions as RollupOptions,
  RolldownBuild as RollupBuild,
}
export type { RollupError, RollupLog, LoggingFunction } from './rollup'
