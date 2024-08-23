import type {
  RolldownOutput,
  RolldownOutputAsset,
  RolldownOutputChunk,
  SourceMap,
} from '../types/rolldown-output'
import type { OutputBundle } from '../types/output-bundle'
import type {
  BindingOutputAsset,
  BindingOutputChunk,
  BindingOutputs,
  FinalBindingOutputs,
} from '../binding'
import {
  AssetSource,
  bindingAssetSource,
  transformAssetSource,
} from './asset-source'

function transformToRollupOutputChunk(
  chunk: BindingOutputChunk,
): RolldownOutputChunk {
  return {
    type: 'chunk',
    get code() {
      return chunk.code
    },
    set code(code: string) {
      chunk.code = code
    },
    fileName: chunk.fileName,
    name: chunk.name,
    get modules() {
      return Object.fromEntries(
        Object.entries(chunk.modules).map(([key, _]) => [key, {}]),
      )
    },
    get imports() {
      return chunk.imports
    },
    set imports(imports: string[]) {
      chunk.imports = imports
    },
    get dynamicImports() {
      return chunk.dynamicImports
    },
    exports: chunk.exports,
    isEntry: chunk.isEntry,
    facadeModuleId: chunk.facadeModuleId || null,
    isDynamicEntry: chunk.isDynamicEntry,
    get moduleIds() {
      return chunk.moduleIds
    },
    get map() {
      return chunk.map ? JSON.parse(chunk.map) : null
    },
    set map(map: SourceMap) {
      chunk.map = JSON.stringify(map)
    },
    sourcemapFileName: chunk.sourcemapFileName || null,
    preliminaryFileName: chunk.preliminaryFileName,
  }
}

function transformToRollupOutputAsset(
  asset: BindingOutputAsset,
): RolldownOutputAsset {
  return {
    type: 'asset',
    fileName: asset.fileName,
    originalFileName: asset.originalFileName || null,
    get source(): AssetSource {
      return transformAssetSource(asset.source)
    },
    set source(source: AssetSource) {
      asset.source = bindingAssetSource(source)
    },
    name: asset.name ?? undefined,
  }
}

export function transformToRollupOutput(
  output: BindingOutputs | FinalBindingOutputs,
): RolldownOutput {
  const { chunks, assets } = output
  return {
    output: [
      ...chunks.map(transformToRollupOutputChunk),
      ...assets.map(transformToRollupOutputAsset),
    ],
  } as RolldownOutput
}

export function transformToOutputBundle(output: BindingOutputs): OutputBundle {
  const bundle = Object.fromEntries(
    transformToRollupOutput(output).output.map((item) => [item.fileName, item]),
  )
  return new Proxy(bundle, {
    deleteProperty(target, property): boolean {
      if (typeof property === 'string') {
        output.delete(property)
      }
      return true
    },
  })
}
