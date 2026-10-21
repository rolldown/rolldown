import type {
  BindingOutputAsset,
  BindingOutputChunk,
  BindingOutputs,
  JsChangedOutputs,
  JsOutputAsset,
  JsOutputChunk,
} from '../binding.cjs';
import type { MinimalPluginContext } from '../plugin/minimal-plugin-context';
import type { OutputCodeCacheEntry } from '../plugin/plugin-context-data';
import { OutputAssetImpl } from '../types/output-asset-impl';
import type { OutputBundle } from '../types/output-bundle';
import { OutputChunkImpl } from '../types/output-chunk-impl';
import type { OutputAsset, OutputChunk, RolldownOutput, SourceMap } from '../types/rolldown-output';
import { bindingifySourcemap } from '../types/sourcemap';
import { type AssetSource, bindingAssetSource, transformAssetSource } from './asset-source';
import { type BindingCell, shouldEagerlyFreeOutputs } from './threadless-free';
import { snapshotChunkModules, transformChunkModules } from './transform-rendered-chunk';

export function transformToRollupSourceMap(map: string): SourceMap {
  const parsed: Omit<SourceMap, 'toString' | 'toUrl'> = JSON.parse(map);
  const obj: SourceMap = {
    ...parsed,
    toString() {
      return JSON.stringify(obj);
    },
    toUrl() {
      const json = obj.toString();
      // Browsers and workerd without Node compatibility have no `Buffer`. Each chunk is encoded
      // at once (no full-size binary string); a multiple of 3 bytes leaves padding to the last.
      let base64: string;
      if (typeof Buffer === 'function') {
        base64 = Buffer.from(json, 'utf-8').toString('base64');
      } else {
        const bytes = new TextEncoder().encode(json);
        base64 = '';
        for (let i = 0; i < bytes.length; i += 0x7ffe) {
          base64 += btoa(String.fromCharCode(...bytes.subarray(i, i + 0x7ffe)));
        }
      }
      return `data:application/json;charset=utf-8;base64,${base64}`;
    },
  };
  // Preserve Rollup's legacy property for plugins while serializing the standard `ignoreList`.
  Object.defineProperty(obj, 'x_google_ignoreList', {
    get: () => obj.ignoreList,
    set: (value: number[] | undefined) => {
      obj.ignoreList = value;
    },
  });
  return obj;
}

function transformToRollupOutputChunk(bindingChunk: BindingOutputChunk): OutputChunkImpl {
  return new OutputChunkImpl(bindingChunk);
}

// The binding cell of each mutable bundle item, on threadless WASI only (see
// `releaseOutputBundleItems`).
const mutableItemCells: WeakMap<OutputChunk | OutputAsset, BindingCell<unknown>> = new WeakMap();

function transformToMutableRollupOutputChunk(
  bindingChunk: BindingOutputChunk,
  changed: ChangedOutputs,
  codeCache?: Map<string, OutputCodeCacheEntry>,
): OutputChunk {
  const fileName = bindingChunk.getFileName();
  // The getters reach the wrapper only through this cell, so clearing it
  // leaves a kept item holding plain data.
  const cell: BindingCell<BindingOutputChunk> = { value: bindingChunk };
  const binding = (): BindingOutputChunk => {
    if (cell.value === undefined) {
      throw new Error('This output chunk has already released its binding object');
    }
    return cell.value;
  };
  const chunk = {
    type: 'chunk',
    get code() {
      const current = binding();
      if (!codeCache) return current.getCode();
      const identity = current.getCodeIdentity();
      const hit = codeCache.get(fileName);
      if (hit && hit.identity === identity) return hit.code;
      const code = current.getCode();
      codeCache.set(fileName, { identity, holder: current, code });
      return code;
    },
    fileName,
    name: bindingChunk.getName(),
    get modules() {
      // Each getModules() call mints per-module boxes only finalizers reclaim,
      // so snapshot and release them on threadless WASI. Safe because the proxy
      // caches this map and `collectChangedBundle` submits an empty one.
      const bindingModules = binding().getModules();
      return shouldEagerlyFreeOutputs()
        ? snapshotChunkModules(bindingModules)
        : transformChunkModules(bindingModules);
    },
    get imports() {
      return binding().getImports();
    },
    get dynamicImports() {
      return binding().getDynamicImports();
    },
    exports: bindingChunk.getExports(),
    isEntry: bindingChunk.getIsEntry(),
    facadeModuleId: bindingChunk.getFacadeModuleId() || null,
    isDynamicEntry: bindingChunk.getIsDynamicEntry(),
    get moduleIds() {
      return binding().getModuleIds();
    },
    get map() {
      const map = binding().getMap();
      return map ? transformToRollupSourceMap(map) : null;
    },
    sourcemapFileName: bindingChunk.getSourcemapFileName() || null,
    preliminaryFileName: bindingChunk.getPreliminaryFileName(),
  } as OutputChunk;
  const cache: Record<string | symbol, any> = {};
  const proxy = new Proxy(chunk, {
    get(target, p) {
      if (p in cache) {
        return cache[p];
      }
      const value = target[p as keyof OutputChunk];
      cache[p] = value;
      return value;
    },
    set(target, p, newValue): boolean {
      cache[p] = newValue;
      // The name copied at creation: on threadless WASI a kept item outlives
      // its box, and writes go to `cache`, never to `target`.
      changed.updated.add(target.fileName);
      return true;
    },
    has(target, p): boolean {
      if (p in cache) return true;
      return p in target;
    },
  });
  if (shouldEagerlyFreeOutputs()) mutableItemCells.set(proxy, cell);
  return proxy;
}

function transformToRollupOutputAsset(bindingAsset: BindingOutputAsset): OutputAssetImpl {
  return new OutputAssetImpl(bindingAsset);
}

function transformToMutableRollupOutputAsset(
  bindingAsset: BindingOutputAsset,
  changed: ChangedOutputs,
): OutputAsset {
  const cell: BindingCell<BindingOutputAsset> = { value: bindingAsset };
  const asset = {
    type: 'asset',
    fileName: bindingAsset.getFileName(),
    originalFileName: bindingAsset.getOriginalFileName() || null,
    originalFileNames: bindingAsset.getOriginalFileNames(),
    get source(): AssetSource {
      if (cell.value === undefined) {
        throw new Error('This output asset has already released its binding object');
      }
      return transformAssetSource(cell.value.getSource());
    },
    name: bindingAsset.getName() ?? undefined,
    names: bindingAsset.getNames(),
  } as OutputAsset;
  const cache: Record<string | symbol, any> = {};
  const proxy = new Proxy(asset, {
    get(target, p) {
      if (p in cache) {
        return cache[p];
      }
      const value = target[p as keyof OutputAsset];
      cache[p] = value;
      return value;
    },
    set(target, p, newValue): boolean {
      cache[p] = newValue;
      // The name copied at creation: on threadless WASI a kept item outlives
      // its box, and writes go to `cache`, never to `target`.
      changed.updated.add(target.fileName);
      return true;
    },
  });
  if (shouldEagerlyFreeOutputs()) mutableItemCells.set(proxy, cell);
  return proxy;
}

export function transformToRollupOutput(output: BindingOutputs): RolldownOutput {
  const { chunks, assets } = output;
  const chunkItems = chunks.map((chunk) => transformToRollupOutputChunk(chunk));
  const assetItems = assets.map((asset) => transformToRollupOutputAsset(asset));
  const outputItems: (OutputChunk | OutputAsset)[] = [...chunkItems, ...assetItems];
  const transformed = {
    output: outputItems,
  } as RolldownOutput;
  if (output.mangleCache !== undefined) {
    transformed.mangleCache = output.mangleCache;
  }
  if (shouldEagerlyFreeOutputs()) {
    for (const item of [...chunkItems, ...assetItems]) {
      // keepDataAlive: cache every lazy field, then release the native
      // payload. Runs after `output.mangleCache` is copied, so the release is
      // the final native read.
      item.__rolldown_external_memory_handle__(true);
    }
  }
  return transformed;
}

function transformToMutableRollupOutput(
  output: BindingOutputs,
  changed: ChangedOutputs,
  codeCache?: Map<string, OutputCodeCacheEntry>,
): RolldownOutput {
  const { chunks, assets } = output;
  return {
    output: [
      ...chunks.map((chunk) => transformToMutableRollupOutputChunk(chunk, changed, codeCache)),
      ...assets.map((asset) => transformToMutableRollupOutputAsset(asset, changed)),
    ],
  } as RolldownOutput;
}

export function transformToOutputBundle(
  context: MinimalPluginContext,
  output: BindingOutputs,
  changed: ChangedOutputs,
  codeCache?: Map<string, OutputCodeCacheEntry>,
): OutputBundle {
  const bundle = Object.fromEntries(
    transformToMutableRollupOutput(output, changed, codeCache).output.map((item) => [
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
      delete target[property as keyof typeof target];
      return true;
    },
  });
}

// The getter-backed fields of `transformToMutableRollupOutputChunk` /
// `transformToMutableRollupOutputAsset`; every other field is copied up front.
const LAZY_MUTABLE_CHUNK_FIELDS = [
  'code',
  'modules',
  'imports',
  'dynamicImports',
  'moduleIds',
  'map',
] as const satisfies readonly (keyof OutputChunk)[];

/**
 * Read every getter-backed field of the hook's bundle items through their
 * proxies, which cache what they read, then detach each item's binding
 * wrapper. On threadless WASI the items' boxes are dropped when
 * `generateBundle` / `writeBundle` settles, and Rollup lets a plugin keep the
 * bundle past the hook, so a kept item must neither read them afterwards nor
 * hold the wrapper, whose methods pin the instance's Memory. Pass the items as
 * created: one the plugin deleted from the bundle can still be kept.
 * See internal-docs/workerd-managed-instance/implementation.md.
 */
export function releaseOutputBundleItems(items: readonly (OutputChunk | OutputAsset)[]): void {
  for (const item of items) {
    const fields = item.type === 'asset' ? (['source'] as const) : LAZY_MUTABLE_CHUNK_FIELDS;
    for (const field of fields) {
      try {
        void (item as unknown as Record<string, unknown>)[field];
      } catch {
        // A later read of this field throws the released-binding error
        // instead; cleanup must not replace the hook's own outcome.
      }
    }
    const cell = mutableItemCells.get(item);
    if (cell !== undefined) cell.value = undefined;
  }
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
