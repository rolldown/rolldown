import type {
  BindingOutputAsset,
  BindingOutputChunk,
  BindingOutputs,
  JsChangedOutputs,
  JsOutputAsset,
  JsOutputChunk,
} from '../binding';
import type { PluginContext } from '../plugin/plugin-context';
import type { OutputBundle } from '../types/output-bundle';
import type {
  OutputAsset,
  OutputChunk,
  RolldownOutput,
  SourceMap,
} from '../types/rolldown-output';
import { bindingifySourcemap } from '../types/sourcemap';
import {
  type AssetSource,
  bindingAssetSource,
  transformAssetSource,
} from './asset-source';
import { aggregateBindingErrorsIntoError } from './error';
import { transformChunkModules } from './transform-rendered-chunk';

function transformToRollupSourceMap(map: string): SourceMap {
  const parsed: Omit<SourceMap, 'toString' | 'toUrl'> = JSON.parse(map);
  const obj: SourceMap = {
    ...parsed,
    toString() {
      return JSON.stringify(obj);
    },
    toUrl() {
      return `data:application/json;charset=utf-8;base64,${
        Buffer.from(obj.toString(), 'utf-8').toString('base64')
      }`;
    },
  };
  return obj;
}

function transformToRollupOutputChunk(
  bindingChunk: BindingOutputChunk,
  changed?: ChangedOutputs,
): OutputChunk {
  const chunk = {
    type: 'chunk',
    get code() {
      return bindingChunk.code;
    },
    fileName: bindingChunk.fileName,
    name: bindingChunk.name,
    get modules() {
      return transformChunkModules(bindingChunk.modules);
    },
    get imports() {
      return bindingChunk.imports;
    },
    get dynamicImports() {
      return bindingChunk.dynamicImports;
    },
    exports: bindingChunk.exports,
    isEntry: bindingChunk.isEntry,
    facadeModuleId: bindingChunk.facadeModuleId || null,
    isDynamicEntry: bindingChunk.isDynamicEntry,
    get moduleIds() {
      return bindingChunk.moduleIds;
    },
    get map() {
      return bindingChunk.map
        ? transformToRollupSourceMap(bindingChunk.map)
        : null;
    },
    sourcemapFileName: bindingChunk.sourcemapFileName || null,
    preliminaryFileName: bindingChunk.preliminaryFileName,
  } as OutputChunk;
  const cache: Record<string | symbol, any> = {};
  return new Proxy(chunk, {
    get(target, p) {
      if (p in cache) {
        return cache[p];
      }
      const value = target[p as keyof OutputChunk];
      cache[p] = value;
      return value;
    },
    set(_target, p, newValue): boolean {
      cache[p] = newValue;
      changed?.updated.add(bindingChunk.fileName);
      return true;
    },
    has(target, p): boolean {
      if (p in cache) return true;
      return p in target;
    },
  });
}

function transformToRollupOutputAsset(
  bindingAsset: BindingOutputAsset,
  changed?: ChangedOutputs,
): OutputAsset {
  const asset = {
    type: 'asset',
    fileName: bindingAsset.fileName,
    originalFileName: bindingAsset.originalFileName || null,
    originalFileNames: bindingAsset.originalFileNames,
    get source(): AssetSource {
      return transformAssetSource(bindingAsset.source);
    },
    name: bindingAsset.name ?? undefined,
    names: bindingAsset.names,
  } as OutputAsset;
  const cache: Record<string | symbol, any> = {};
  return new Proxy(asset, {
    get(target, p) {
      if (p in cache) {
        return cache[p];
      }
      const value = target[p as keyof OutputAsset];
      cache[p] = value;
      return value;
    },
    set(_target, p, newValue): boolean {
      cache[p] = newValue;
      changed?.updated.add(bindingAsset.fileName);
      return true;
    },
  });
}

export function transformToRollupOutput(
  output: BindingOutputs,
  changed?: ChangedOutputs,
): RolldownOutput {
  handleOutputErrors(output);
  const { chunks, assets } = output;
  return {
    output: [
      ...chunks.map((chunk) => transformToRollupOutputChunk(chunk, changed)),
      ...assets.map((asset) => transformToRollupOutputAsset(asset, changed)),
    ],
  } as RolldownOutput;
}

export function handleOutputErrors(output: BindingOutputs): void {
  const rawErrors = output.errors;
  if (rawErrors.length > 0) {
    throw aggregateBindingErrorsIntoError(rawErrors);
  }
}

export function transformToOutputBundle(
  context: PluginContext,
  output: BindingOutputs,
  changed: ChangedOutputs,
): OutputBundle {
  const bundle = Object.fromEntries(
    transformToRollupOutput(output, changed).output.map((item) => [
      item.fileName,
      item,
    ]),
  );
  return new Proxy(bundle, {
    set(_target, _p, _newValue, _receiver) {
      const originalStackTraceLimit = Error.stackTraceLimit;
      Error.stackTraceLimit = 2;
      const message =
        'This plugin assigns to bundle variable. This is discouraged by Rollup and is not supported by Rolldown. This will be ignored. https://rollupjs.org/plugin-development/#generatebundle:~:text=DANGER,this.emitFile.';
      const stack = new Error(message).stack ?? message;
      Error.stackTraceLimit = originalStackTraceLimit;

      context.warn({
        message: stack,
        code: 'UNSUPPORTED_BUNDLE_ASSIGNMENT',
      });
      return true;
    },
    deleteProperty(target, property): boolean {
      if (typeof property === 'string') {
        changed.deleted.add(property);
      }
      return true;
    },
  });
}

export interface ChangedOutputs {
  updated: Set<string>;
  deleted: Set<string>;
}

// TODO find a way only transfer the changed part to Rust side.
export function collectChangedBundle(
  changed: ChangedOutputs,
  bundle: OutputBundle,
): JsChangedOutputs {
  const changes: Record<string, JsOutputChunk | JsOutputAsset> = {};
  for (const key in bundle) {
    if (changed.deleted.has(key) || !changed.updated.has(key)) {
      continue;
    }
    const item = bundle[key];
    if (item.type === 'asset') {
      changes[key] = {
        filename: item.fileName,
        originalFileNames: item.originalFileNames,
        source: bindingAssetSource(item.source),
        names: item.names,
      };
    } else {
      // not all properties modifications are reflected to Rust side
      changes[key] = {
        code: item.code,
        filename: item.fileName,
        name: item.name,
        isEntry: item.isEntry,
        exports: item.exports,
        modules: {},
        imports: item.imports,
        dynamicImports: item.dynamicImports,
        facadeModuleId: item.facadeModuleId || undefined,
        isDynamicEntry: item.isDynamicEntry,
        moduleIds: item.moduleIds,
        map: bindingifySourcemap(item.map),
        sourcemapFilename: item.sourcemapFileName || undefined,
        preliminaryFilename: item.preliminaryFileName,
      };
    }
  }
  return {
    changes,
    deleted: changed.deleted,
  };
}
