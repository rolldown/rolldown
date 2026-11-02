import type { BindingRenderedModule } from '../binding.cjs';
import type { RenderedModule } from '../types/rolldown-output';

export function transformToRenderedModule(
  bindingRenderedModule: BindingRenderedModule,
): RenderedModule {
  return {
    get code() {
      return bindingRenderedModule.code;
    },
    get renderedLength() {
      return bindingRenderedModule.renderedLength;
    },
    get renderedExports() {
      return bindingRenderedModule.renderedExports;
    },
  };
}
