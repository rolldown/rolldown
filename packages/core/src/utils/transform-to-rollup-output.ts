import type { AsyncReturnType } from 'type-fest'
import { Bundler, OutputChunk } from '@rolldown/node-binding'
import type {
  RollupOutput,
  OutputChunk as RollupOutputChunk,
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

export function transformToRollupOutput(
  output: AsyncReturnType<Bundler['write']>,
): RollupOutput {
  const [first, ...rest] = output
  return {
    output: [
      transformToRollupOutputChunk(first),
      ...rest.map(transformToRollupOutputChunk),
    ],
  }
}
