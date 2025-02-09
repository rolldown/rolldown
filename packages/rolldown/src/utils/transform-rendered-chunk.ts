import { RenderedChunk as BindingRenderedChunk } from '../binding'
import { RenderedChunk } from '../types/rolldown-output'
import { transformToRenderedModule } from './transform-rendered-module'

export function transformRenderedChunk(
  chunk: BindingRenderedChunk,
): RenderedChunk {
  return {
    get name() {
      return chunk.name
    },
    get isEntry() {
      return chunk.isEntry
    },
    get isDynamicEntry() {
      return chunk.isDynamicEntry
    },
    get facadeModuleId() {
      return chunk.facadeModuleId
    },
    get moduleIds() {
      return chunk.moduleIds
    },
    get exports() {
      return chunk.exports
    },
    get fileName() {
      return chunk.fileName
    },
    get imports() {
      return chunk.imports
    },
    get dynamicImports() {
      return chunk.dynamicImports
    },
    get modules() {
      return transformChunkModules(chunk.modules)
    },
  }
}

export function transformChunkModules(
  modules: BindingRenderedChunk['modules'],
): RenderedChunk['modules'] {
  const result: RenderedChunk['modules'] = {}
  for (let i = 0; i < modules.values.length; i++) {
    let key = modules.keys[i]
    const mod = modules.values[i]
    result[key] = transformToRenderedModule(mod)
  }
  return result
}
