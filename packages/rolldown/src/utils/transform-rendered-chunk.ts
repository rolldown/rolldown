import {
  BindingChunkModules,
  RenderedChunk as BindingRenderedChunk,
} from '../binding'
import { RolldownRenderedChunk } from '../types/rolldown-output'
import { transformToRenderedModule } from './transform-rendered-module'

export function transformRenderedChunk(
  chunk: BindingRenderedChunk,
): RolldownRenderedChunk {
  return {
    ...chunk,
    get modules() {
      return transformChunkModules(chunk.modules)
    },
  }
}

export function transformChunkModules(
  modules: BindingChunkModules,
): RolldownRenderedChunk['modules'] {
  const result: RolldownRenderedChunk['modules'] = {}
  for (const [id, mod] of modules.toEntries()) {
    result[id] = transformToRenderedModule(mod)
  }
  return result
}
