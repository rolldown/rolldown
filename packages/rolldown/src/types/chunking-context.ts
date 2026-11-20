import type { BindingChunkingContext } from '../binding.cjs';
import type { LogHandler } from '../log/log-handler';
import { LOG_LEVEL_WARN } from '../log/logging';
import { logModuleSideEffectsWriteAfterBuildEnd } from '../log/logs';
import type { PluginContextData } from '../plugin/plugin-context-data';
import { transformModuleInfo } from '../utils/transform-module-info';
import type { ModuleInfo } from './module-info';

export class ChunkingContextImpl {
  private moduleInfoCache: Map<string, ModuleInfo> | undefined = new Map();

  constructor(
    private context: BindingChunkingContext,
    private pluginContextData: PluginContextData,
    private onLog: LogHandler,
  ) {}

  private warnWriteAfterBuildEnd = (moduleId: string): void => {
    this.onLog(LOG_LEVEL_WARN, logModuleSideEffectsWriteAfterBuildEnd(moduleId));
  };

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
      const info = transformModuleInfo(
        bindingInfo,
        option,
        this.context,
        this.pluginContextData.moduleSideEffectsOption,
        this.warnWriteAfterBuildEnd,
      );
      this.moduleInfoCache?.set(moduleId, info);
      return info;
    }
    return null;
  }
}
