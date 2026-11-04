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
  let assigned: Set<string | symbol> | undefined;
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
      if (!assigned) {
        const names = new Set<string | symbol>();
        assigned = names;
        changed.updated.set(fileName, () => collectChunkChange(bindingChunk, chunk, cache, names));
      }
      assigned.add(p);
      return true;
    },
    has(target, p): boolean {
      if (p in cache) return true;
      return p in target;
    },
  });
}

/**
 * Of the chunk properties that `update_output_chunk` in Rust applies, the ones a plugin changed,
 * or `undefined` when it changed none.
 *
 * A property the plugin neither read nor assigned is not in `cache` and still holds Rust's value,
 * so it is left out. An unread `map` is therefore not serialized in Rust, parsed here, and parsed
 * again in Rust. A chunk with no change is not sent at all, so Rust keeps the same chunk data and
 * `outputCodeCache` keeps its code. This matters for Vite's `vite:css-post`: when a build has a
 * pure CSS chunk, it assigns `chunk.imports` of every chunk, mostly to the same file names.
 */
function collectChunkChange(
  bindingChunk: BindingOutputChunk,
  original: OutputChunk,
  cache: Record<string | symbol, any>,
  assigned: Set<string | symbol>,
): JsOutputChunk | undefined {
  const change: JsOutputChunk = {
    filename: 'fileName' in cache ? cache.fileName : original.fileName,
    isEntry: 'isEntry' in cache ? cache.isEntry : original.isEntry,
  };
  let isChanged = change.filename !== original.fileName || change.isEntry !== original.isEntry;
  // A string the plugin read cannot have changed.
  if (assigned.has('code')) {
    change.code = cache.code;
    isChanged = true;
  }
  // A map or an array the plugin read may have been changed in place.
  if ('map' in cache) {
    change.map = cache.map == null ? null : bindingifySourcemap(cache.map);
    isChanged = true;
  }
  if ('imports' in cache && !isSameStrings(cache.imports, bindingChunk.getImports())) {
    change.imports = cache.imports;
    isChanged = true;
  }
  if (
    'dynamicImports' in cache &&
    !isSameStrings(cache.dynamicImports, bindingChunk.getDynamicImports())
  ) {
    change.dynamicImports = cache.dynamicImports;
    isChanged = true;
  }
  return isChanged ? change : undefined;
}

function isSameStrings(value: unknown, original: string[]): boolean {
  if (!Array.isArray(value) || value.length !== original.length) return false;
  for (let i = 0; i < original.length; i++) {
    if (value[i] !== original[i]) return false;
  }
  return true;
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
      cache[p] = value;
      return value;
    },
    set(_target, p, newValue): boolean {
      cache[p] = newValue;
      changed.updated.set(asset.fileName, undefined);
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
  /**
   * The file names of the outputs a plugin assigned a property of. For a chunk, the value returns
   * the properties that changed (see `collectChunkChange`).
   */
  updated: Map<string, (() => JsOutputChunk | undefined) | undefined>;
  deleted: Set<string>;
}

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
      const change = changed.updated.get(key)?.();
      if (change) {
        changes[key] = change;
      }
    }
  }
  return {
    changes,
    deleted: changed.deleted,
  };
}
