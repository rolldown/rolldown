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
import type { Extends, TypeAssert } from '../types/assert';
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
    get immutable() {
      return bindingChunk.getImmutable();
    },
    fileName,
    // Metadata is read lazily: the bundle is rebuilt for every JS `generateBundle`/`writeBundle`
    // hook, and most hooks read only a few fields of a few outputs. Each read crosses NAPI, and
    // the Proxy below caches the first read, so a field is still fetched at most once per hook.
    get name() {
      return bindingChunk.getName();
    },
    get modules() {
      return transformChunkModules(bindingChunk.getModules());
    },
    get imports() {
      return bindingChunk.getImports();
    },
    get dynamicImports() {
      return bindingChunk.getDynamicImports();
    },
    get exports() {
      return bindingChunk.getExports();
    },
    get isEntry() {
      return bindingChunk.getIsEntry();
    },
    get facadeModuleId() {
      return bindingChunk.getFacadeModuleId() || null;
    },
    get isDynamicEntry() {
      return bindingChunk.getIsDynamicEntry();
    },
    get moduleIds() {
      return bindingChunk.getModuleIds();
    },
    get map() {
      const map = bindingChunk.getMap();
      return map ? transformToRollupSourceMap(map) : null;
    },
    get sourcemapFileName() {
      return bindingChunk.getSourcemapFileName() || null;
    },
    get preliminaryFileName() {
      return bindingChunk.getPreliminaryFileName();
    },
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
      if (p === 'fileName' && newValue !== (cache.fileName ?? fileName)) {
        if (newValue === fileName && !assigned?.has('immutable')) delete cache.immutable;
        else cache.immutable = false;
      }
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
 * Leaving an unread `map` out avoids serializing it in Rust, parsing it here, and parsing it again
 * in Rust. A chunk with no change is not sent at all, so Rust keeps the same chunk data and
 * `outputCodeCache` keeps its code. This matters for Vite's `vite:css-post`: when a build has a
 * pure CSS chunk, it assigns `chunk.imports` of every chunk, mostly to the same file names.
 */
function collectChunkChange(
  bindingChunk: BindingOutputChunk,
  original: OutputChunk,
  cache: Record<string | symbol, any>,
  assigned: Set<string | symbol>,
): JsOutputChunk | undefined {
  // Fails type-check when `JsOutputChunk` gains a field that is not listed here.
  // List a field only once this function sends it.
  type _ = TypeAssert<
    Extends<
      keyof JsOutputChunk,
      'filename' | 'isEntry' | 'immutable' | 'code' | 'map' | 'imports' | 'dynamicImports'
    >
  >;
  const change: JsOutputChunk = {
    filename: 'fileName' in cache ? cache.fileName : original.fileName,
    isEntry: 'isEntry' in cache ? cache.isEntry : original.isEntry,
  };
  let isChanged = change.filename !== original.fileName || change.isEntry !== original.isEntry;
  if (
    assigned.has('immutable') &&
    cache.immutable !== (change.filename === original.fileName && original.immutable)
  ) {
    change.immutable = cache.immutable;
    isChanged = true;
  }
  // A string the plugin read cannot have changed.
  if (assigned.has('code')) {
    if (typeof cache.code !== 'string') {
      throw invalidChunkProperty(original.fileName, 'code', 'a string', cache.code);
    }
    change.code = cache.code;
    isChanged = true;
  }
  // A map or an array the plugin read may have been changed in place.
  if ('map' in cache) {
    change.map = cache.map == null ? null : bindingifySourcemap(cache.map);
    isChanged = true;
  }
  if ('imports' in cache && !isSameStrings(cache.imports, bindingChunk.getImports())) {
    if (!Array.isArray(cache.imports)) {
      throw invalidChunkProperty(original.fileName, 'imports', 'an array', cache.imports);
    }
    change.imports = cache.imports;
    isChanged = true;
  }
  if (
    'dynamicImports' in cache &&
    !isSameStrings(cache.dynamicImports, bindingChunk.getDynamicImports())
  ) {
    if (!Array.isArray(cache.dynamicImports)) {
      throw invalidChunkProperty(
        original.fileName,
        'dynamicImports',
        'an array',
        cache.dynamicImports,
      );
    }
    change.dynamicImports = cache.dynamicImports;
    isChanged = true;
  }
  return isChanged ? change : undefined;
}

// Rust keeps the old value of a field set to `undefined`, so a wrong type is rejected here.
function invalidChunkProperty(
  fileName: string,
  property: string,
  expected: string,
  value: unknown,
): TypeError {
  return new TypeError(
    `The \`${property}\` property of chunk "${fileName}" must be ${expected}. Received ${value === null ? 'null' : typeof value}.`,
  );
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
  const fileName = bindingAsset.getFileName();
  const asset = {
    type: 'asset',
    get immutable() {
      return bindingAsset.getImmutable();
    },
    fileName,
    // Lazy for the same reason as chunk metadata above.
    get originalFileName() {
      return bindingAsset.getOriginalFileName() || null;
    },
    get originalFileNames() {
      return bindingAsset.getOriginalFileNames();
    },
    get source(): AssetSource {
      return transformAssetSource(bindingAsset.getSource());
    },
    get name() {
      return bindingAsset.getName() ?? undefined;
    },
    get names() {
      return bindingAsset.getNames();
    },
  } as OutputAsset;
  const cache: Record<string | symbol, any> = {};
  let immutableAssigned = false;
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
      if (p === 'immutable') immutableAssigned = true;
      if (p === 'fileName' && newValue !== (cache.fileName ?? fileName)) {
        if (newValue === fileName && !immutableAssigned) delete cache.immutable;
        else cache.immutable = false;
      }
      cache[p] = newValue;
      if (p !== 'immutable' || changed.updated.has(fileName) || newValue !== asset.immutable) {
        changed.updated.set(fileName, immutableAssigned);
      }
      return true;
    },
  });
}

/**
 * The native side of an output is only freed when V8 collects its JS wrapper. Telling V8 how much
 * memory that is keeps it from waiting forever on an almost empty JS heap.
 */
export function trackOutputExternalMemory(output: BindingOutputs): void {
  for (const chunk of output.chunks) chunk.trackExternalMemory();
  for (const asset of output.assets) asset.trackExternalMemory();
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
   * For a chunk, the value returns the changed properties. For an asset, it records whether
   * the plugin assigned `immutable`.
   */
  updated: Map<string, (() => JsOutputChunk | undefined) | boolean>;
  deleted: Set<string>;
}

export function collectChangedBundle(
  changed: ChangedOutputs,
  bundle: OutputBundle,
): JsChangedOutputs {
  const changes: Record<string, JsOutputChunk | JsOutputAsset> = {};
  for (const key in bundle) {
    const update = changed.updated.get(key);
    if (changed.deleted.has(key) || update === undefined) {
      continue;
    }
    const item = bundle[key];
    if (item.type === 'asset') {
      changes[key] = {
        filename: item.fileName,
        immutable: update === true ? item.immutable : undefined,
        originalFileNames: item.originalFileNames,
        source: bindingAssetSource(item.source),
        names: item.names,
      };
    } else if (typeof update === 'function') {
      const change = update();
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
