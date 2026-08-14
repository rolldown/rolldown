import type { BindingChunkingContext } from '../binding.cjs';
import type { PluginContextData } from '../plugin/plugin-context-data';
import { transformModuleInfo } from '../utils/transform-module-info';
import type { ModuleInfo } from './module-info';

export class ChunkingContextImpl {
  constructor(
    private context: BindingChunkingContext,
    private pluginContextData: PluginContextData,
  ) {}
  getModuleInfo(moduleId: string): ModuleInfo | null {
    const bindingInfo = this.context.getModuleInfo(moduleId);
    if (bindingInfo) {
      const info = transformModuleInfo(
        bindingInfo,
        this.pluginContextData.getModuleOption(moduleId),
      );
      return info;
    }
    return null;
  }
}
