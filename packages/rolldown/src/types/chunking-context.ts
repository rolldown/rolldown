import type { BindingChunkingContext } from '../binding.cjs';
import type { PluginContextData } from '../plugin/plugin-context-data';
import { shouldEagerlyFreeOutputs } from '../utils/threadless-free';
import { snapshotModuleInfo, transformModuleInfo } from '../utils/transform-module-info';
import type { ModuleInfo } from './module-info';

export class ChunkingContextImpl {
  constructor(
    private context: BindingChunkingContext,
    private pluginContextData: PluginContextData,
  ) {}
  getModuleInfo(moduleId: string): ModuleInfo | null {
    const bindingInfo = this.context.getModuleInfo(moduleId);
    if (bindingInfo) {
      // Each call mints a fresh module-info box retaining the module's full
      // source, and the threadless flavor never runs GC finalizers, so hand
      // out a plain-data snapshot and release the box immediately.
      const info = shouldEagerlyFreeOutputs()
        ? snapshotModuleInfo(bindingInfo, this.pluginContextData.getModuleOption(moduleId))
        : transformModuleInfo(bindingInfo, this.pluginContextData.getModuleOption(moduleId));
      return info;
    }
    return null;
  }
}
