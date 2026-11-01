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
import { transformChunkModules } from './transform-rendered-chunk';

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

function transformToRollupOutputChunk(bindingChunk: BindingOutputChunk): OutputChunk {
  return new OutputChunkImpl(bindingChunk);
}

function arrayChanged(value: ArrayLike<unknown>, snapshot: ArrayLike<unknown>): boolean {
  if (value.length !== snapshot.length) return true;
  for (let i = 0; i < snapshot.length; i++) {
    if (value[i] !== snapshot[i]) return true;
  }
  return false;
}

function trackNestedMutations(value: unknown, fileName: string, changed: ChangedOutputs): void {
  if (value === null || typeof value !== 'object' || changed.updated.has(fileName)) return;

  // Snapshot only properties a plugin reads. Keep the actual arrays and typed arrays so
  // their identity, native methods, and writes through subarray()/buffer views still work.
  let hasChanged: () => boolean;
  if (value instanceof Uint8Array || Array.isArray(value)) {
    // Buffer.slice() shares its backing storage, so explicitly copy binary sources.
    const snapshot = value instanceof Uint8Array ? new Uint8Array(value) : value.slice();
    hasChanged = () => arrayChanged(value, snapshot);
  } else {
    // Source maps contain scalar fields and arrays; also include the legacy ignoreList alias.
    const map = value as Record<string, unknown>;
    const snapshot = Object.getOwnPropertyNames(map).map((key) => {
      const item = map[key];
      return [key, Array.isArray(item) ? item.slice() : item] as const;
    });
    hasChanged = () =>
      Object.getOwnPropertyNames(map).length !== snapshot.length ||
      snapshot.some(([key, item]) =>
        Array.isArray(item)
          ? !Array.isArray(map[key]) || arrayChanged(map[key], item)
          : map[key] !== item,
      );
  }

  const checks = (changed.mutationChecks ??= new Map());
  const existing = checks.get(fileName);
  if (existing) existing.push(hasChanged);
  else checks.set(fileName, [hasChanged]);
}

function transformToMutableRollupOutputChunk(
  bindingChunk: BindingOutputChunk,
  changed: ChangedOutputs,
  codeCache?: Map<string, OutputCodeCacheEntry>,
): OutputChunk {
  const fileName = bindingChunk.getFileName();
  const chunk = {
    type: 'chunk',
    get code() {
      if (!codeCache) return bindingChunk.getCode();
      const identity = bindingChunk.getCodeIdentity();
      const hit = codeCache.get(fileName);
      if (hit && hit.identity === identity) return hit.code;
      const code = bindingChunk.getCode();
      codeCache.set(fileName, { identity, holder: bindingChunk, code });
      return code;
    },
    fileName,
    name: bindingChunk.getName(),
    get modules() {
      return transformChunkModules(bindingChunk.getModules());
    },
    get imports() {
      return bindingChunk.getImports();
    },
    get dynamicImports() {
      return bindingChunk.getDynamicImports();
    },
    exports: bindingChunk.getExports(),
    isEntry: bindingChunk.getIsEntry(),
    facadeModuleId: bindingChunk.getFacadeModuleId() || null,
    isDynamicEntry: bindingChunk.getIsDynamicEntry(),
    get moduleIds() {
      return bindingChunk.getModuleIds();
    },
    get map() {
      const map = bindingChunk.getMap();
      return map ? transformToRollupSourceMap(map) : null;
    },
    sourcemapFileName: bindingChunk.getSourcemapFileName() || null,
    preliminaryFileName: bindingChunk.getPreliminaryFileName(),
  } as OutputChunk;
  const cache: Record<string | symbol, any> = {};
  return new Proxy(chunk, {
    get(target, p) {
      if (p in cache) {
        return cache[p];
      }
      const value = target[p as keyof OutputChunk];
      // These are the mutable fields applied by update_output_chunk on the Rust side.
      if (p === 'imports' || p === 'dynamicImports' || p === 'map') {
        trackNestedMutations(value, fileName, changed);
      }
      cache[p] = value;
      return value;
    },
    set(_target, p, newValue): boolean {
      cache[p] = newValue;
      changed.updated.add(bindingChunk.getFileName());
      return true;
    },
    has(target, p): boolean {
      if (p in cache) return true;
      return p in target;
    },
  });
}

function transformToRollupOutputAsset(bindingAsset: BindingOutputAsset): OutputAsset {
  return new OutputAssetImpl(bindingAsset);
}

function transformToMutableRollupOutputAsset(
  bindingAsset: BindingOutputAsset,
  changed: ChangedOutputs,
): OutputAsset {
  const asset = {
    type: 'asset',
    fileName: bindingAsset.getFileName(),
    originalFileName: bindingAsset.getOriginalFileName() || null,
    originalFileNames: bindingAsset.getOriginalFileNames(),
    get source(): AssetSource {
      return transformAssetSource(bindingAsset.getSource());
    },
    name: bindingAsset.getName() ?? undefined,
    names: bindingAsset.getNames(),
  } as OutputAsset;
  const cache: Record<string | symbol, any> = {};
  return new Proxy(asset, {
    get(target, p) {
      if (p in cache) {
        return cache[p];
      }
      const value = target[p as keyof OutputAsset];
      if (p === 'source' || p === 'names' || p === 'originalFileNames') {
        trackNestedMutations(value, asset.fileName, changed);
      }
      cache[p] = value;
      return value;
    },
    set(_target, p, newValue): boolean {
      cache[p] = newValue;
      changed.updated.add(bindingAsset.getFileName());
      return true;
    },
  });
}

export function transformToRollupOutput(output: BindingOutputs): RolldownOutput {
  const { chunks, assets } = output;
  const transformed = {
    output: [
      ...chunks.map((chunk) => transformToRollupOutputChunk(chunk)),
      ...assets.map((asset) => transformToRollupOutputAsset(asset)),
    ],
  } as RolldownOutput;
  if (output.mangleCache !== undefined) {
    transformed.mangleCache = output.mangleCache;
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

export interface ChangedOutputs {
  updated: Set<string>;
  deleted: Set<string>;
  mutationChecks?: Map<string, (() => boolean)[]>;
}

// TODO find a way only transfer the changed part to Rust side.
export function collectChangedBundle(
  changed: ChangedOutputs,
  bundle: OutputBundle,
): JsChangedOutputs {
  const changes: Record<string, JsOutputChunk | JsOutputAsset> = {};
  for (const key in bundle) {
    if (changed.deleted.has(key)) continue;
    if (!changed.updated.has(key)) {
      if (!changed.mutationChecks?.get(key)?.some((hasChanged) => hasChanged())) continue;
      // Avoid snapshots of the remaining properties read while serializing this output.
      changed.updated.add(key);
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
