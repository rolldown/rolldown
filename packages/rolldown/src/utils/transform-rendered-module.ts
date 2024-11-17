import { RenderedModule } from '../binding'

export function transformToRenderedModule(
  bindingRenderedModule: RenderedModule,
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
