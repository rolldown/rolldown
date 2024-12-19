import { RenderedChunk as BindingRenderedChunk } from '../binding'
import { RenderedChunk } from '../types/rolldown-output'
import { transformToRenderedModule } from './transform-rendered-module'

export function transformRenderedChunk(
  chunk: BindingRenderedChunk,
): RenderedChunk {
  return {
    ...chunk,
    get modules() {
      return transformChunkModules(chunk.modules)
    },
  }
}

export function transformChunkModules(
  modules: BindingRenderedChunk['modules'],
): RenderedChunk['modules'] {
  const result: RenderedChunk['modules'] = {}
  for (const [id, mod] of Object.entries(modules)) {
    result[id] = transformToRenderedModule(mod)
  }
  return result
}
