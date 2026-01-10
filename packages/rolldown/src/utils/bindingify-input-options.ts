import {
  BindingAttachDebugInfo,
  BindingChunkModuleOrderBy,
  BindingLogLevel,
  BindingPropertyReadSideEffects,
  BindingPropertyWriteSideEffects,
} from '../binding.cjs';
import type {
  BindingDeferSyncScanData,
  BindingExperimentalOptions,
  BindingInjectImportNamed,
  BindingInjectImportNamespace,
  BindingInputOptions,
} from '../binding.cjs';
import { bindingifyManifestPlugin, BuiltinPlugin } from '../builtin-plugin/utils';
import { bindingifyBuiltInPlugin } from '../builtin-plugin/utils';
import type { LogHandler } from '../log/log-handler';
import type { LogLevelOption } from '../log/logging';
import type { AttachDebugOptions, DevModeOptions, InputOptions } from '../options/input-options';
import type { OutputOptions } from '../options/output-options';
import type { RolldownPlugin } from '../plugin';
import { bindingifyPlugin } from '../plugin/bindingify-plugin';
import { PluginContextData } from '../plugin/plugin-context-data';
import { arraify } from './misc';
import { normalizedStringOrRegex } from './normalize-string-or-regex';
import {
  type NormalizedTransformOptions,
  normalizeTransformOptions,
} from './normalize-transform-options';

export function bindingifyInputOptions(
  rawPlugins: RolldownPlugin[],
  inputOptions: InputOptions,
  outputOptions: OutputOptions,
  normalizedOutputPlugins: RolldownPlugin[],
  onLog: LogHandler,
  logLevel: LogLevelOption,
  watchMode: boolean,
): BindingInputOptions {
  const pluginContextData = new PluginContextData(onLog, outputOptions, normalizedOutputPlugins);

  const plugins = rawPlugins.map((plugin) => {
    if ('_parallel' in plugin) {
      return undefined;
    }
    if (plugin instanceof BuiltinPlugin) {
      switch (plugin.name) {
        case 'builtin:vite-manifest':
          return bindingifyManifestPlugin(plugin, pluginContextData);
        default:
          return bindingifyBuiltInPlugin(plugin);
      }
    }
    return bindingifyPlugin(
      plugin,
      inputOptions,
      outputOptions,
      pluginContextData,
      normalizedOutputPlugins,
      onLog,
      logLevel,
      watchMode,
    );
  });

  // Normalize transform options to extract define, inject, and oxc transform options
  const normalizedTransform = normalizeTransformOptions(inputOptions);

  return {
    input: bindingifyInput(inputOptions.input),
    plugins,
    cwd: inputOptions.cwd ?? process.cwd(),
    external: bindingifyExternal(inputOptions.external),
    resolve: bindingifyResolve(inputOptions.resolve),
    platform: inputOptions.platform,
    shimMissingExports: inputOptions.shimMissingExports,
    logLevel: bindingifyLogLevel(logLevel),
    // convert to async function to handle errors thrown in onLog
    onLog: async (level, log) => onLog(level, log),
    // After normalized, `false` will be converted to `undefined`, otherwise, default value will be assigned
    // Because it is hard to represent Enum in napi, ref: https://github.com/napi-rs/napi-rs/issues/507
    // So we use `undefined | NormalizedTreeshakingOptions` (or Option<NormalizedTreeshakingOptions> in Rust side), to represent `false | NormalizedTreeshakingOptions`
    treeshake: bindingifyTreeshakeOptions(inputOptions.treeshake),
    moduleTypes: inputOptions.moduleTypes,
    define: normalizedTransform.define,
    inject: bindingifyInject(normalizedTransform.inject),
    experimental: bindingifyExperimental(inputOptions.experimental),
    profilerNames: outputOptions.generatedCode?.profilerNames,
    transform: normalizedTransform.oxcTransformOptions,
    watch: bindingifyWatch(inputOptions.watch),
    dropLabels: normalizedTransform.dropLabels,
    keepNames: outputOptions.keepNames,
    checks: inputOptions.checks,
    deferSyncScanData: () => {
      let ret: BindingDeferSyncScanData[] = [];
      pluginContextData.moduleOptionMap.forEach((value, key) => {
        if (value.invalidate) {
          ret.push({
            id: key,
            sideEffects: value.moduleSideEffects ?? undefined,
          });
        }
      });
      return ret;
    },
    makeAbsoluteExternalsRelative: bindingifyMakeAbsoluteExternalsRelative(
      inputOptions.makeAbsoluteExternalsRelative,
    ),
    devtools: inputOptions.devtools,
    invalidateJsSideCache: pluginContextData.clear.bind(pluginContextData),
    preserveEntrySignatures: bindingifyPreserveEntrySignatures(
      inputOptions.preserveEntrySignatures,
    ),
    optimization: inputOptions.optimization,
    context: inputOptions.context,
    tsconfig: inputOptions.resolve?.tsconfigFilename ?? inputOptions.tsconfig,
  };
}

function bindingifyDevMode(devMode?: DevModeOptions): BindingExperimentalOptions['devMode'] {
  if (devMode) {
    if (typeof devMode === 'boolean') {
      return devMode ? {} : undefined;
    }
    return devMode;
  }
}

function bindingifyAttachDebugInfo(
  attachDebugInfo?: AttachDebugOptions,
): BindingExperimentalOptions['attachDebugInfo'] {
  switch (attachDebugInfo) {
    case undefined:
      return undefined;
    case 'full':
      return BindingAttachDebugInfo.Full;
    case 'simple':
      return BindingAttachDebugInfo.Simple;
    case 'none':
      return BindingAttachDebugInfo.None;
  }
}

function bindingifyExternal(external: InputOptions['external']): BindingInputOptions['external'] {
  if (external) {
    if (typeof external === 'function') {
      return (id, importer, isResolved) => {
        if (id.startsWith('\0')) return false;
        return external(id, importer, isResolved) ?? false;
      };
    }
    return arraify(external);
  }
}

function bindingifyExperimental(
  experimental: InputOptions['experimental'],
): BindingInputOptions['experimental'] {
  let chunkModulesOrder = BindingChunkModuleOrderBy.ExecOrder;
  if (experimental?.chunkModulesOrder) {
    switch (experimental.chunkModulesOrder) {
      case 'exec-order':
        chunkModulesOrder = BindingChunkModuleOrderBy.ExecOrder;
        break;
      case 'module-id':
        chunkModulesOrder = BindingChunkModuleOrderBy.ModuleId;
        break;
      default:
        throw new Error(`Unexpected chunkModulesOrder: ${experimental.chunkModulesOrder}`);
    }
  }
  return {
    strictExecutionOrder: experimental?.strictExecutionOrder,
    viteMode: experimental?.viteMode,
    resolveNewUrlToAsset: experimental?.resolveNewUrlToAsset,
    devMode: bindingifyDevMode(experimental?.devMode),
    attachDebugInfo: bindingifyAttachDebugInfo(experimental?.attachDebugInfo),
    chunkModulesOrder,
    chunkImportMap: experimental?.chunkImportMap,
    onDemandWrapping: experimental?.onDemandWrapping,
    incrementalBuild: experimental?.incrementalBuild,
    nativeMagicString: experimental?.nativeMagicString,
    chunkOptimization: experimental?.chunkOptimization,
  };
}

function bindingifyResolve(resolve: InputOptions['resolve']): BindingInputOptions['resolve'] {
  // process is undefined for browser build
  const yarnPnp = typeof process === 'object' && !!process.versions?.pnp;
  if (resolve) {
    const { alias, extensionAlias, ...rest } = resolve;
    return {
      alias: alias
        ? Object.entries(alias).map(([name, replacement]) => ({
            find: name,
            replacements: replacement === false ? [undefined] : arraify(replacement),
          }))
        : undefined,
      extensionAlias: extensionAlias
        ? Object.entries(extensionAlias).map(([name, value]) => ({
            target: name,
            replacements: value,
          }))
        : undefined,
      yarnPnp,
      ...rest,
    };
  } else {
    return {
      yarnPnp,
    };
  }
}

function bindingifyInject(
  inject: NormalizedTransformOptions['inject'],
): BindingInputOptions['inject'] {
  if (inject) {
    return Object.entries(inject).map(
      ([alias, item]): BindingInjectImportNamed | BindingInjectImportNamespace => {
        if (Array.isArray(item)) {
          // import * as fs from 'node:fs'
          // fs: ['node:fs', '*' ],
          if (item[1] === '*') {
            return {
              tagNamespace: true,
              alias,
              from: item[0],
            };
          }

          // import { Promise } from 'es6-promise'
          // Promise: [ 'es6-promise', 'Promise' ],

          // import { Promise as P } from 'es6-promise'
          // P: [ 'es6-promise', 'Promise' ],
          return {
            tagNamed: true,
            alias,
            from: item[0],
            imported: item[1],
          };
        } else {
          // import $ from 'jquery'
          // $: 'jquery',

          // 'Object.assign': path.resolve( 'src/helpers/object-assign.js' ),
          return {
            tagNamed: true,
            imported: 'default',
            alias,
            from: item,
          };
        }
      },
    );
  }
}

function bindingifyLogLevel(logLevel: InputOptions['logLevel']): BindingInputOptions['logLevel'] {
  switch (logLevel) {
    case 'silent':
      return BindingLogLevel.Silent;
    case 'debug':
      return BindingLogLevel.Debug;
    case 'warn':
      return BindingLogLevel.Warn;
    case 'info':
      return BindingLogLevel.Info;
    default:
      throw new Error(`Unexpected log level: ${logLevel}`);
  }
}

function bindingifyInput(input: InputOptions['input']): BindingInputOptions['input'] {
  if (input === undefined) {
    return [];
  }

  if (typeof input === 'string') {
    return [{ import: input }];
  }

  if (Array.isArray(input)) {
    return input.map((src) => ({ import: src }));
  }

  return Object.entries(input).map(([name, import_path]) => {
    return { name, import: import_path };
  });
}

function bindingifyWatch(watch: InputOptions['watch']): BindingInputOptions['watch'] {
  if (watch) {
    return {
      buildDelay: watch.buildDelay,
      skipWrite: watch.skipWrite,
      include: normalizedStringOrRegex(watch.include),
      exclude: normalizedStringOrRegex(watch.exclude),
      onInvalidate: (...args) => watch.onInvalidate?.(...args),
    };
  }
}

function bindingifyTreeshakeOptions(
  config: InputOptions['treeshake'],
): BindingInputOptions['treeshake'] {
  if (config === false) {
    return undefined;
  }

  if (config === true || config === undefined) {
    return {
      moduleSideEffects: true,
    };
  }

  let normalizedConfig: BindingInputOptions['treeshake'] = {
    moduleSideEffects: true,
    annotations: config.annotations,
    manualPureFunctions: config.manualPureFunctions,
    unknownGlobalSideEffects: config.unknownGlobalSideEffects,
    commonjs: config.commonjs,
  };
  switch (config.propertyReadSideEffects) {
    case 'always':
      normalizedConfig.propertyReadSideEffects = BindingPropertyReadSideEffects.Always;
      break;
    case false:
      normalizedConfig.propertyReadSideEffects = BindingPropertyReadSideEffects.False;
      break;
    default:
  }
  switch (config.propertyWriteSideEffects) {
    case 'always':
      normalizedConfig.propertyWriteSideEffects = BindingPropertyWriteSideEffects.Always;
      break;
    case false:
      normalizedConfig.propertyWriteSideEffects = BindingPropertyWriteSideEffects.False;
      break;
    default:
  }
  if (config.moduleSideEffects === undefined) {
    normalizedConfig.moduleSideEffects = true;
  } else if (config.moduleSideEffects === 'no-external') {
    normalizedConfig.moduleSideEffects = [
      { external: true, sideEffects: false },
      { external: false, sideEffects: true },
    ];
  } else {
    normalizedConfig.moduleSideEffects = config.moduleSideEffects;
  }

  return normalizedConfig;
}

function bindingifyMakeAbsoluteExternalsRelative(
  makeAbsoluteExternalsRelative: InputOptions['makeAbsoluteExternalsRelative'],
): BindingInputOptions['makeAbsoluteExternalsRelative'] {
  if (makeAbsoluteExternalsRelative === 'ifRelativeSource') {
    return { type: 'IfRelativeSource' };
  }
  if (typeof makeAbsoluteExternalsRelative === 'boolean') {
    return { type: 'Bool', field0: makeAbsoluteExternalsRelative };
  }
}

export function bindingifyPreserveEntrySignatures(
  preserveEntrySignatures: InputOptions['preserveEntrySignatures'],
): BindingInputOptions['preserveEntrySignatures'] {
  if (preserveEntrySignatures == undefined) {
    return undefined;
  } else if (typeof preserveEntrySignatures === 'string') {
    return { type: 'String', field0: preserveEntrySignatures };
  } else {
    return { type: 'Bool', field0: preserveEntrySignatures };
  }
}
