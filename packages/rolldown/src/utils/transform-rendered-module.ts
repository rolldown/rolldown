import { BindingRenderedModule } from '../binding'
import { RolldownRenderedModule } from '../types/rolldown-output'

export function transformToRenderedModule(
  mod: BindingRenderedModule,
): RolldownRenderedModule {
  // cache getter?
  let code: string | null
  return {
    get code() {
      if (typeof code === 'undefined') {
        code = mod.code
      }
      return code
    },
    get renderedLength() {
      if (typeof code === 'undefined') {
        code = mod.code
      }
      return code?.length || 0
    },
  }
}
