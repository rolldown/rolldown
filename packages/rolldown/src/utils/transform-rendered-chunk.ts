import { RenderedChunk } from '../binding'
import { RolldownRenderedChunk } from '../types/rolldown-output'
import { transformToRenderedModule } from './transform-rendered-module'

export function transformRenderedChunk(
  chunk: RenderedChunk,
): RolldownRenderedChunk {
  return {
    ...chunk,
    get modules() {
      return transformChunkModules(chunk.modules)
    },
  }
}

export function transformChunkModules(
  modules: RenderedChunk['modules'],
): RolldownRenderedChunk['modules'] {
  const result: RolldownRenderedChunk['modules'] = {}
  for (const [id, mod] of Object.entries(modules)) {
    result[id] = transformToRenderedModule(mod)
  }
  return result
}
