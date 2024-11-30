import { BindingRenderedModule } from '../binding'
import { RolldownRenderedModule } from '../types/rolldown-output'

export function transformToRenderedModule(
  bindingRenderedModule: BindingRenderedModule,
): RolldownRenderedModule {
  return {
    get code() {
      return bindingRenderedModule.code
    },
    get renderedLength() {
      return bindingRenderedModule.code?.length || 0
    },
  }
}
