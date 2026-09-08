import type { BindingChunkingContext } from '../binding.cjs';
import type { PluginContextData } from '../plugin/plugin-context-data';
import { transformModuleInfo } from '../utils/transform-module-info';
import type { ModuleInfo } from './module-info';

export class ChunkingContextImpl {
  private moduleInfoCache: Map<string, ModuleInfo> | undefined = new Map();

  constructor(
    private context: BindingChunkingContext,
    private pluginContextData: PluginContextData,
  ) {}

  clearModuleInfoCache(): void {
    this.moduleInfoCache = undefined;
  }

  getModuleInfo(moduleId: string): ModuleInfo | null {
    const cached = this.moduleInfoCache?.get(moduleId);
    if (cached) {
      return cached;
    }
    const bindingInfo = this.context.getModuleInfo(moduleId);
    if (bindingInfo) {
      const option = this.pluginContextData.getModuleOption(moduleId);
      const info = transformModuleInfo(bindingInfo, option);
      if (this.moduleInfoCache) {
        Object.defineProperty(info, 'moduleSideEffects', {
          get: () => option.moduleSideEffects,
          set: (moduleSideEffects: ModuleInfo['moduleSideEffects']) => {
            option.moduleSideEffects = moduleSideEffects;
            option.invalidate = true;
          },
        });
        this.moduleInfoCache.set(moduleId, info);
      }
      return info;
    }
    return null;
  }
}
