import type {
  SourceMap,
  RolldownOutput,
  RolldownOutputAsset,
  RolldownOutputChunk,
} from '../types/rolldown-output'
import type { OutputBundle } from '../types/output-bundle'
import type {
  BindingOutputAsset,
  BindingOutputChunk,
  BindingOutputs,
  JsChangedOutputs,
  JsOutputAsset,
  JsOutputChunk,
} from '../binding'
import {
  AssetSource,
  bindingAssetSource,
  transformAssetSource,
} from './asset-source'
import { bindingifySourcemap } from '../types/sourcemap'
import { normalizeErrors } from './error'
import { transformChunkModules } from './transform-rendered-chunk'
import { Buffer } from 'node:buffer'

function transformToRollupSourceMap(map: string): SourceMap {
  const parsed: Omit<SourceMap, 'toString' | 'toUrl'> = JSON.parse(map)
  const obj: SourceMap = {
    ...parsed,
    toString() {
      return JSON.stringify(obj)
    },
    toUrl() {
      return `data:application/json;charset=utf-8;base64,${Buffer.from(obj.toString(), 'utf-8').toString('base64')}`
    },
  }
  return obj
}

function transformToRollupOutputChunk(
  bindingChunk: BindingOutputChunk,
  changed?: ChangedOutputs,
): RolldownOutputChunk {
  const chunk = {
    type: 'chunk',
    get code() {
      return bindingChunk.code
    },
    fileName: bindingChunk.fileName,
    name: bindingChunk.name,
    get modules() {
      return transformChunkModules(bindingChunk.modules)
    },
    get imports() {
      return bindingChunk.imports
    },
    get dynamicImports() {
      return bindingChunk.dynamicImports
    },
    exports: bindingChunk.exports,
    isEntry: bindingChunk.isEntry,
    facadeModuleId: bindingChunk.facadeModuleId || null,
    isDynamicEntry: bindingChunk.isDynamicEntry,
    get moduleIds() {
      return bindingChunk.moduleIds
    },
    get map() {
      return bindingChunk.map
        ? transformToRollupSourceMap(bindingChunk.map)
        : null
    },
    sourcemapFileName: bindingChunk.sourcemapFileName || null,
    preliminaryFileName: bindingChunk.preliminaryFileName,
  } as RolldownOutputChunk
  const cache: Record<string | symbol, any> = {}
  return new Proxy(chunk, {
    get(target, p) {
      if (p in cache) {
        return cache[p]
      }
      return target[p as keyof RolldownOutputChunk]
    },
    set(target, p, newValue): boolean {
      cache[p] = newValue
      changed?.updated.add(bindingChunk.fileName)
      return true
    },
  })
}

function transformToRollupOutputAsset(
  bindingAsset: BindingOutputAsset,
  changed?: ChangedOutputs,
): RolldownOutputAsset {
  const asset = {
    type: 'asset',
    fileName: bindingAsset.fileName,
    originalFileName: bindingAsset.originalFileName || null,
    originalFileNames: bindingAsset.originalFileNames,
    get source(): AssetSource {
      return transformAssetSource(bindingAsset.source)
    },
    name: bindingAsset.name ?? undefined,
    names: bindingAsset.names,
  } as RolldownOutputAsset
  const cache: Record<string | symbol, any> = {}
  return new Proxy(asset, {
    get(target, p) {
      if (p in cache) {
        return cache[p]
      }
      return target[p as keyof RolldownOutputAsset]
    },
    set(target, p, newValue): boolean {
      cache[p] = newValue
      changed?.updated.add(bindingAsset.fileName)
      return true
    },
  })
}

export function transformToRollupOutput(
  output: BindingOutputs,
  changed?: ChangedOutputs,
): RolldownOutput {
  handleOutputErrors(output)
  const { chunks, assets } = output
  return {
    output: [
      ...chunks.map((chunk) => transformToRollupOutputChunk(chunk, changed)),
      ...assets.map((asset) => transformToRollupOutputAsset(asset, changed)),
    ],
  } as RolldownOutput
}

export function handleOutputErrors(output: BindingOutputs) {
  const rawErrors = output.errors
  if (rawErrors.length > 0) {
    throw normalizeErrors(rawErrors)
  }
}

export function transformToOutputBundle(
  output: BindingOutputs,
  changed: ChangedOutputs,
): OutputBundle {
  const bundle = Object.fromEntries(
    transformToRollupOutput(output, changed).output.map((item) => [
      item.fileName,
      item,
    ]),
  )
  return new Proxy(bundle, {
    deleteProperty(target, property): boolean {
      if (typeof property === 'string') {
        changed.deleted.add(property)
      }
      return true
    },
  })
}

export interface ChangedOutputs {
  updated: Set<string>
  deleted: Set<string>
}

// TODO find a way only transfer the changed part to rust side.
export function collectChangedBundle(
  changed: ChangedOutputs,
  bundle: OutputBundle,
): JsChangedOutputs {
  const assets: Array<JsOutputAsset> = []
  const chunks: Array<JsOutputChunk> = []

  for (const key in bundle) {
    if (changed.deleted.has(key) || !changed.updated.has(key)) {
      continue
    }
    const item = bundle[key]
    if (item.type === 'asset') {
      assets.push({
        filename: item.fileName,
        originalFileNames: item.originalFileNames,
        source: bindingAssetSource(item.source),
        names: item.names,
      })
    } else {
      // not all properties modifications are reflected to rust side
      chunks.push({
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
      })
    }
  }
  return {
    assets,
    chunks,
    deleted: Array.from(changed.deleted),
  }
}
