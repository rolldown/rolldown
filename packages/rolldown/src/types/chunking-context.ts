import type { BindingChunkingContext } from '../binding';
import { transformModuleInfo } from '../utils/transform-module-info';
import type { ModuleInfo } from './module-info';

export class ChunkingContextImpl {
  constructor(private context: BindingChunkingContext) {}
  getModuleInfo(moduleId: string): ModuleInfo | null {
    const bindingInfo = this.context.getModuleInfo(moduleId);
    if (bindingInfo) {
      const info = transformModuleInfo(bindingInfo, {
        // TODO(hyf0): I don't know why we have to need these to transform the module info.
        moduleSideEffects: null,
        meta: {},
      });
      return info;
    }
    return null;
  }
}
