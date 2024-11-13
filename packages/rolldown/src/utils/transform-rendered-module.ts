import { RenderedModule } from '../types/rendered-module'
import { BindingRenderedModule } from '../binding'

export function transformToRenderedModule(
  bindingRenderedModule: BindingRenderedModule,
): RenderedModule {
  return {
    get code() {
      return bindingRenderedModule.code
    },
    get renderedLength() {
      return bindingRenderedModule.code?.length || 0
    },
  }
}
