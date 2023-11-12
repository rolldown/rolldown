import type { AsyncReturnType } from 'type-fest'
import { Bundler, OutputAsset, OutputChunk } from '@rolldown/node-binding'
import type {
  RollupOutput,
  OutputChunk as RollupOutputChunk,
  OutputAsset as RollupOutputAsset,
} from '../rollup-types'
import { unimplemented } from '.'

function transformToRollupOutputChunk(chunk: OutputChunk): RollupOutputChunk {
  return {
    type: 'chunk',
    code: chunk.code,
    fileName: chunk.fileName,
    get dynamicImports() {
      return unimplemented()
    },
    get implicitlyLoadedBefore() {
      return unimplemented()
    },
    get importedBindings() {
      return unimplemented()
    },
    get imports() {
      return unimplemented()
    },
    get modules() {
      return unimplemented()
    },
    get referencedFiles() {
      return unimplemented()
    },
    get map() {
      return unimplemented()
    },
    get exports() {
      return unimplemented()
    },
    get facadeModuleId() {
      return chunk.facadeModuleId || null
    },
    get isDynamicEntry() {
      return unimplemented()
    },
    get isEntry() {
      return chunk.isEntry
    },
    get isImplicitEntry() {
      return unimplemented()
    },
    get moduleIds() {
      return unimplemented()
    },
    get name() {
      return unimplemented()
    },
    get sourcemapFileName() {
      return unimplemented()
    },
    get preliminaryFileName() {
      return unimplemented()
    },
  }
}

function transformToRollupOutputAsset(asset: OutputAsset) : RollupOutputAsset {
  return {
    type: 'asset',
    fileName: asset.fileName,
    source: asset.source,
    get name() {
      return unimplemented()
    },
    get needsCodeReference() {
      return unimplemented()
    }
  }
}

export function transformToRollupOutput(
  output: AsyncReturnType<Bundler['write']>,
): RollupOutput {
  const { chunks, assets } = output;

  return {
    // @ts-expect-error here chunks.length > 0
    output: [
      ...chunks.map(transformToRollupOutputChunk),
      ...assets.map(transformToRollupOutputAsset),
    ],
  }
}
