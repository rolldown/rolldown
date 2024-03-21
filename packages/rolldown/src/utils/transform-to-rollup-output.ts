import {
  RolldownOutput,
  RolldownOutputAsset,
  RolldownOutputChunk,
} from '../objects/rolldown-output'
import { OutputBundle } from '../objects/output-bundle'
import {
  BindingOutputAsset,
  BindingOutputChunk,
  BindingOutputs,
} from '../binding'

function transformToRollupOutputChunk(
  chunk: BindingOutputChunk,
): RolldownOutputChunk {
  return {
    type: 'chunk',
    code: chunk.code,
    fileName: chunk.fileName,
    modules: Object.fromEntries(
      Object.entries(chunk.modules).map(([key, _]) => [key, {}]),
    ),
    exports: chunk.exports,
    isEntry: chunk.isEntry,
    facadeModuleId: chunk.facadeModuleId || null,
    isDynamicEntry: chunk.isDynamicEntry,
    moduleIds: chunk.moduleIds,
    map: chunk.map ? JSON.parse(chunk.map) : null,
    sourcemapFileName: chunk.sourcemapFileName || null,
  }
}

function transformToRollupOutputAsset(
  asset: BindingOutputAsset,
): RolldownOutputAsset {
  return {
    type: 'asset',
    fileName: asset.fileName,
    source: asset.source,
  }
}

export function transformToRollupOutput(
  output: BindingOutputs,
): RolldownOutput {
  const { chunks, assets } = output
  const [firstChunk, ...restChunks] = chunks
  return {
    output: [
      transformToRollupOutputChunk(firstChunk),
      ...restChunks.map(transformToRollupOutputChunk),
      ...assets.map(transformToRollupOutputAsset),
    ],
  }
}

export function transformToOutputBundle(output: BindingOutputs): OutputBundle {
  return Object.fromEntries(
    transformToRollupOutput(output).output.map((item) => [item.fileName, item]),
  )
}
