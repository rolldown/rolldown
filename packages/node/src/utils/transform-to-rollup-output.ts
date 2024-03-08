import { OutputAsset, OutputChunk, Outputs } from '@rolldown/node-binding'
import type {
  RollupOutput,
  OutputChunk as RollupOutputChunk,
  OutputAsset as RollupOutputAsset,
  OutputBundle,
} from '../rollup-types'
import { unimplemented } from '.'

function transformToRollupOutputChunk(chunk: OutputChunk): RollupOutputChunk {
  return {
    type: 'chunk',
    code: chunk.code,
    fileName: chunk.fileName,
    // @ts-expect-error undefined can't assign to null
    modules: chunk.modules,
    exports: chunk.exports,
    isEntry: chunk.isEntry,
    facadeModuleId: chunk.facadeModuleId || null,
    isDynamicEntry: chunk.isDynamicEntry,
    moduleIds: chunk.moduleIds,
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
    get referencedFiles() {
      return unimplemented()
    },
    get map() {
      return unimplemented()
    },
    get isImplicitEntry() {
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

function transformToRollupOutputAsset(asset: OutputAsset): RollupOutputAsset {
  return {
    type: 'asset',
    fileName: asset.fileName,
    source: asset.source,
    get name() {
      return unimplemented()
    },
    get needsCodeReference() {
      return unimplemented()
    },
  }
}

export function transformToRollupOutput(output: Outputs): RollupOutput {
  const { chunks, assets } = output

  if (chunks.length === 0) {
    throw new Error("No output chunks found. At least one OutputChunk is required.");
  }
  
  return {
    output: [
      ...chunks.map(transformToRollupOutputChunk),
      ...assets.map(transformToRollupOutputAsset),
    ] as [OutputChunk, ...(OutputChunk | OutputAsset)[]],
  }
}

type RolldownOutputChunk = OutputChunk & { type: 'chunk' }
type RolldownOutputAsset = OutputAsset & { type: 'asset' }
export interface RolldownOutput {
  output: [
    RolldownOutputChunk,
    ...(RolldownOutputChunk | RolldownOutputAsset)[],
  ]
}

export function transformToOutputBundle(output: Outputs): OutputBundle {
  return Object.fromEntries(
    transformToRollupOutput(output).output.map((item) => [item.fileName, item]),
  )
}
