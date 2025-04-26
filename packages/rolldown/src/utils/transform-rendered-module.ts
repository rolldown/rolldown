import type { BindingRenderedModule } from '../binding';
import type { RenderedModule } from '../types/rolldown-output';

export function transformToRenderedModule(
  bindingRenderedModule: BindingRenderedModule,
): RenderedModule {
  return {
    get code() {
      return bindingRenderedModule.code;
    },
    get renderedLength() {
      return bindingRenderedModule.code?.length || 0;
    },
    get renderedExports() {
      return bindingRenderedModule.renderedExports;
    },
  };
}
